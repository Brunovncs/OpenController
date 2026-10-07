//! A thin wrapper over the parts of SDL 3 that read gamepads.
//!
//! SDL does the hard work of talking to each controller in its own protocol: the DualShock 4
//! and DualSense over USB, Bluetooth and the Sony receiver, Switch Pro and Joy-Con, Xbox pads
//! over XInput, and hundreds of generic pads through the bundled mapping database. Everything
//! here must run on the one thread that called [`Sdl::init`]: SDL reports hot-plugging only to
//! that thread, and keeping all calls there avoids contention on its joystick lock. Rumble is
//! the exception: each controller gets it from a thread of its own (see [`Rumbler`]).

use crate::device::{PadType, Power, SdlConnection};
use crate::extras::{Features, TOUCH_LEFT, TOUCH_RIGHT, TOUCH_TWO};
use crate::mapping::{PadState, axis, button};
use crate::motion::{Finger, touch_buttons};
use crate::report::{Facts, model_path};
use sdl3_sys::everything::*;
use std::ffi::{CStr, c_char};
use std::marker::PhantomData;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

static MAPPINGS: &str = include_str!("../assets/gamecontrollerdb.txt");

/// A rumble request that takes this long means a controller that hardly answers (one behind a
/// stuck uinput driver takes 30 s): it gets no more, so it cannot hold SDL up again.
const RUMBLE_STALL: Duration = Duration::from_secs(5);
/// Rumble threads of closed controllers, which must be done before SDL quits.
static LEAVING: Mutex<Vec<JoinHandle<()>>> = Mutex::new(Vec::new());

fn text(p: *const c_char) -> Option<String> {
    (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()).filter(|s| !s.is_empty())
}

pub fn error() -> String {
    text(SDL_GetError()).unwrap_or_else(|| "unknown SDL error".into())
}

/// The linked SDL version, such as `3.4.18`.
pub fn version() -> String {
    let v = SDL_GetVersion();
    format!("{}.{}.{}", v / 1_000_000, v / 1000 % 1000, v % 1000)
}

pub enum Event {
    GamepadAdded(u32),
    GamepadRemoved(u32),
    /// A new input report from the gamepad has been applied.
    GamepadUpdated(u32),
    Battery(u32),
    JoystickAdded(u32),
    JoystickRemoved(u32),
    Other,
}

/// SDL, initialised for gamepads only. Not `Send`: it stays on the thread that created it.
pub struct Sdl {
    _thread_bound: PhantomData<*const ()>,
}

impl Sdl {
    pub fn init() -> Result<Sdl, String> {
        let hints: [(*const c_char, &CStr); 6] = [
            // There is no window to have focus; controllers must be read while a game has it.
            (SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS, c"1"),
            // Full reports from PlayStation controllers on Bluetooth, which carry everything.
            (SDL_HINT_JOYSTICK_ENHANCED_REPORTS, c"1"),
            // Player number on the DualSense and Switch lights, set from the XInput slot.
            (SDL_HINT_JOYSTICK_HIDAPI_PS5_PLAYER_LED, c"1"),
            (SDL_HINT_JOYSTICK_HIDAPI_SWITCH_PLAYER_LED, c"1"),
            (SDL_HINT_JOYSTICK_HIDAPI_SWITCH_HOME_LED, c"1"),
            // A DualShock 3 behind DsHidMini in its SXS mode, which stands in for Sony's
            // sixaxis.sys: without that driver Windows cannot start the controller at all.
            (SDL_HINT_JOYSTICK_HIDAPI_PS3_SIXAXIS_DRIVER, c"1"),
        ];
        unsafe {
            for (name, value) in hints {
                SDL_SetHint(name, value.as_ptr());
            }
            if !SDL_Init(SDL_INIT_GAMEPAD) {
                return Err(error());
            }
            // State is read whole after each report (UPDATE_COMPLETE), so the per-axis and
            // per-button events, dozens per report, would only be queued and thrown away.
            for ty in [
                SDL_EVENT_JOYSTICK_AXIS_MOTION,
                SDL_EVENT_JOYSTICK_BALL_MOTION,
                SDL_EVENT_JOYSTICK_HAT_MOTION,
                SDL_EVENT_JOYSTICK_BUTTON_DOWN,
                SDL_EVENT_JOYSTICK_BUTTON_UP,
                SDL_EVENT_GAMEPAD_AXIS_MOTION,
                SDL_EVENT_GAMEPAD_BUTTON_DOWN,
                SDL_EVENT_GAMEPAD_BUTTON_UP,
                SDL_EVENT_GAMEPAD_TOUCHPAD_DOWN,
                SDL_EVENT_GAMEPAD_TOUCHPAD_MOTION,
                SDL_EVENT_GAMEPAD_TOUCHPAD_UP,
                SDL_EVENT_GAMEPAD_SENSOR_UPDATE,
            ] {
                SDL_SetEventEnabled(ty.0, false);
            }
            let io = SDL_IOFromConstMem(MAPPINGS.as_ptr().cast(), MAPPINGS.len());
            SDL_AddGamepadMappingsFromIO(io, true);
        }
        Ok(Sdl { _thread_bound: PhantomData })
    }

    pub fn poll(&self) -> Option<Event> {
        let mut ev = SDL_Event::default();
        if !unsafe { SDL_PollEvent(&mut ev) } {
            return None;
        }
        let ty = SDL_EventType(unsafe { ev.r#type });
        Some(unsafe {
            match ty {
                SDL_EVENT_GAMEPAD_ADDED => Event::GamepadAdded(ev.gdevice.which.0),
                SDL_EVENT_GAMEPAD_REMOVED => Event::GamepadRemoved(ev.gdevice.which.0),
                SDL_EVENT_GAMEPAD_UPDATE_COMPLETE => Event::GamepadUpdated(ev.gdevice.which.0),
                SDL_EVENT_JOYSTICK_BATTERY_UPDATED => Event::Battery(ev.jbattery.which.0),
                SDL_EVENT_JOYSTICK_ADDED => Event::JoystickAdded(ev.jdevice.which.0),
                SDL_EVENT_JOYSTICK_REMOVED => Event::JoystickRemoved(ev.jdevice.which.0),
                _ => Event::Other,
            }
        })
    }

    pub fn is_gamepad(&self, id: u32) -> bool {
        unsafe { SDL_IsGamepad(SDL_JoystickID(id)) }
    }

    /// Name, vendor and product of a joystick SDL has no gamepad mapping for.
    pub fn joystick_info(&self, id: u32) -> (String, u16, u16) {
        let id = SDL_JoystickID(id);
        unsafe {
            (
                text(SDL_GetJoystickNameForID(id)).unwrap_or_else(|| "Unknown controller".into()),
                SDL_GetJoystickVendorForID(id),
                SDL_GetJoystickProductForID(id),
            )
        }
    }

    /// What SDL says about a joystick it has no gamepad mapping for, opened for a moment if it
    /// is not open already.
    pub fn joystick_facts(&self, id: u32) -> Facts {
        let jid = SDL_JoystickID(id);
        unsafe {
            let open = SDL_GetJoystickFromID(jid);
            let js = if open.is_null() { SDL_OpenJoystick(jid) } else { open };
            let mut f = Facts {
                vendor: SDL_GetJoystickVendorForID(jid),
                product: SDL_GetJoystickProductForID(jid),
                version: SDL_GetJoystickProductVersionForID(jid),
                joystick_name: text(SDL_GetJoystickNameForID(jid)).unwrap_or_default(),
                guid: guid_string(SDL_GetJoystickGUIDForID(jid)),
                path: model_path(&text(SDL_GetJoystickPathForID(jid)).unwrap_or_default()),
                joystick_type: joystick_type(SDL_GetJoystickTypeForID(jid)).into(),
                ..Facts::default()
            };
            if !js.is_null() {
                joystick_counts(js, &mut f);
                if open.is_null() {
                    SDL_CloseJoystick(js);
                }
            }
            f
        }
    }

    pub fn open(&self, id: u32) -> Option<Gamepad> {
        let gp = unsafe { SDL_OpenGamepad(SDL_JoystickID(id)) };
        (!gp.is_null()).then(|| {
            let path = text(unsafe { SDL_GetGamepadPath(gp) }).unwrap_or_default();
            Gamepad { gp, rumbler: Rumbler::start(SDL_JoystickID(id), path) }
        })
    }
}

#[derive(Default)]
struct RumbleRequest {
    /// The latest rumble asked for and not sent yet: low, high, milliseconds.
    want: Option<(u16, u16, u32)>,
    stop: bool,
}

/// Sends one controller's rumble from a thread of its own, so a controller slow to take it
/// never holds up the input thread. Only the latest request waits; older ones are dropped.
struct Rumbler {
    shared: Arc<(Mutex<RumbleRequest>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}

impl Rumbler {
    fn start(id: SDL_JoystickID, path: String) -> Rumbler {
        let shared = Arc::new((Mutex::new(RumbleRequest::default()), Condvar::new()));
        let theirs = shared.clone();
        let thread = std::thread::Builder::new().name("open-controller-rumble".into()).spawn(move || Rumbler::run(id, &path, &theirs)).ok();
        Rumbler { shared, thread }
    }

    fn run(id: SDL_JoystickID, path: &str, shared: &(Mutex<RumbleRequest>, Condvar)) {
        let (lock, wake) = shared;
        let mut route = Route::of(path);
        let mut stalled = false;
        loop {
            let (low, high, ms) = {
                let Ok(mut r) = lock.lock() else { return };
                loop {
                    if let Some(w) = r.want.take() {
                        break w;
                    }
                    if r.stop {
                        return;
                    }
                    let Ok(next) = wake.wait(r) else { return };
                    r = next;
                }
            };
            if stalled {
                continue;
            }
            let asked = Instant::now();
            route.send(id, low, high, ms);
            stalled = asked.elapsed() >= RUMBLE_STALL;
        }
    }

    fn send(&self, low: u16, high: u16, ms: u32) {
        let (lock, wake) = &*self.shared;
        if let Ok(mut r) = lock.lock() {
            r.want = Some((low, high, ms));
            wake.notify_one();
        }
    }
}

/// How a controller's rumble gets to it.
enum Route {
    Sdl,
    /// Straight to the event node of a controller another program makes through uinput, which
    /// SDL would wait on with every controller held (see `linux::ff`).
    #[cfg(target_os = "linux")]
    Node(crate::linux::ff::Rumble),
}

impl Route {
    #[cfg(target_os = "linux")]
    fn of(path: &str) -> Route {
        use crate::linux::ff;
        match ff::is_userspace(path).then(|| ff::Rumble::open(path)).flatten() {
            Some(r) => Route::Node(r),
            None => Route::Sdl,
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn of(_: &str) -> Route {
        Route::Sdl
    }

    fn send(&mut self, id: SDL_JoystickID, low: u16, high: u16, ms: u32) {
        match self {
            // The gamepad may have been closed meanwhile: SDL then finds no gamepad for the id,
            // or refuses the pointer, as it checks every object it is given.
            Route::Sdl => unsafe {
                let gp = SDL_GetGamepadFromID(id);
                if !gp.is_null() {
                    SDL_RumbleGamepad(gp, low, high, ms);
                }
            },
            #[cfg(target_os = "linux")]
            Route::Node(r) => {
                let _ = r.set(low, high, ms);
            }
        }
    }
}

impl Drop for Rumbler {
    /// Not waited for here: the input thread must not wait on a controller that does not answer.
    /// SDL quits only once every rumble thread is done.
    fn drop(&mut self) {
        let (lock, wake) = &*self.shared;
        if let Ok(mut r) = lock.lock() {
            r.stop = true;
            wake.notify_one();
        }
        if let (Some(t), Ok(mut leaving)) = (self.thread.take(), LEAVING.lock()) {
            leaving.retain(|t| !t.is_finished());
            leaving.push(t);
        }
    }
}

fn guid_string(g: SDL_GUID) -> String {
    let mut buf = [0 as c_char; 33];
    unsafe { SDL_GUIDToString(g, buf.as_mut_ptr(), buf.len() as i32) };
    text(buf.as_ptr()).unwrap_or_default()
}

fn joystick_type(t: SDL_JoystickType) -> &'static str {
    match t {
        SDL_JOYSTICK_TYPE_GAMEPAD => "gamepad",
        SDL_JOYSTICK_TYPE_WHEEL => "wheel",
        SDL_JOYSTICK_TYPE_ARCADE_STICK => "arcade stick",
        SDL_JOYSTICK_TYPE_FLIGHT_STICK => "flight stick",
        SDL_JOYSTICK_TYPE_DANCE_PAD => "dance pad",
        SDL_JOYSTICK_TYPE_GUITAR => "guitar",
        SDL_JOYSTICK_TYPE_DRUM_KIT => "drum kit",
        SDL_JOYSTICK_TYPE_ARCADE_PAD => "arcade pad",
        SDL_JOYSTICK_TYPE_THROTTLE => "throttle",
        _ => "unknown",
    }
}

fn gamepad_type(t: SDL_GamepadType) -> String {
    text(unsafe { SDL_GetGamepadStringForType(t) }).unwrap_or_else(|| "unknown".into())
}

/// # Safety
/// `js` must be an open joystick.
unsafe fn joystick_counts(js: *mut SDL_Joystick, f: &mut Facts) {
    unsafe {
        f.axes = SDL_GetNumJoystickAxes(js);
        f.buttons = SDL_GetNumJoystickButtons(js);
        f.hats = SDL_GetNumJoystickHats(js);
        f.balls = SDL_GetNumJoystickBalls(js);
    }
}

impl Drop for Sdl {
    fn drop(&mut self) {
        let leaving = LEAVING.lock().map(|mut l| std::mem::take(&mut *l)).unwrap_or_default();
        for t in leaving {
            let _ = t.join();
        }
        unsafe { SDL_Quit() };
    }
}

pub struct Gamepad {
    gp: *mut SDL_Gamepad,
    rumbler: Rumbler,
}

const BUTTONS: u32 = button::COUNT;

impl Gamepad {
    pub fn name(&self) -> String {
        text(unsafe { SDL_GetGamepadName(self.gp) }).unwrap_or_else(|| "Controller".into())
    }

    pub fn vendor(&self) -> u16 {
        unsafe { SDL_GetGamepadVendor(self.gp) }
    }

    pub fn product(&self) -> u16 {
        unsafe { SDL_GetGamepadProduct(self.gp) }
    }

    pub fn serial(&self) -> Option<String> {
        text(unsafe { SDL_GetGamepadSerial(self.gp) })
    }

    pub fn path(&self) -> String {
        text(unsafe { SDL_GetGamepadPath(self.gp) }).unwrap_or_default()
    }

    pub fn pad_type(&self) -> PadType {
        match unsafe { SDL_GetGamepadType(self.gp) } {
            SDL_GAMEPAD_TYPE_STANDARD => PadType::Standard,
            SDL_GAMEPAD_TYPE_XBOX360 => PadType::Xbox360,
            SDL_GAMEPAD_TYPE_XBOXONE => PadType::XboxOne,
            SDL_GAMEPAD_TYPE_PS3 => PadType::Ps3,
            SDL_GAMEPAD_TYPE_PS4 => PadType::Ps4,
            SDL_GAMEPAD_TYPE_PS5 => PadType::Ps5,
            SDL_GAMEPAD_TYPE_NINTENDO_SWITCH_PRO => PadType::SwitchPro,
            SDL_GAMEPAD_TYPE_NINTENDO_SWITCH_JOYCON_LEFT => PadType::JoyConLeft,
            SDL_GAMEPAD_TYPE_NINTENDO_SWITCH_JOYCON_RIGHT => PadType::JoyConRight,
            SDL_GAMEPAD_TYPE_NINTENDO_SWITCH_JOYCON_PAIR => PadType::JoyConPair,
            SDL_GAMEPAD_TYPE_GAMECUBE => PadType::GameCube,
            _ => PadType::Unknown,
        }
    }

    pub fn connection(&self) -> SdlConnection {
        match unsafe { SDL_GetGamepadConnectionState(self.gp) } {
            SDL_JOYSTICK_CONNECTION_WIRED => SdlConnection::Wired,
            SDL_JOYSTICK_CONNECTION_WIRELESS => SdlConnection::Wireless,
            _ => SdlConnection::Unknown,
        }
    }

    pub fn power(&self) -> Power {
        let mut pct = -1;
        let state = unsafe { SDL_GetGamepadPowerInfo(self.gp, &mut pct) };
        let pct = (0..=100).contains(&pct).then_some(pct as u8);
        match state {
            SDL_POWERSTATE_ON_BATTERY => Power::Battery(pct),
            SDL_POWERSTATE_CHARGING => Power::Charging(pct),
            SDL_POWERSTATE_CHARGED => Power::Charged,
            SDL_POWERSTATE_NO_BATTERY => Power::Wired,
            _ => Power::Unknown,
        }
    }

    pub fn has_button(&self, b: u32) -> bool {
        unsafe { SDL_GamepadHasButton(self.gp, SDL_GamepadButton(b as i32)) }
    }

    pub fn features(&self) -> Features {
        unsafe {
            let props = SDL_GetGamepadProperties(self.gp);
            let cap = |name| SDL_GetBooleanProperty(props, name, false);
            Features {
                touchpad: SDL_GetNumGamepadTouchpads(self.gp) > 0,
                motion: SDL_GamepadHasSensor(self.gp, SDL_SENSOR_GYRO),
                rumble: cap(SDL_PROP_GAMEPAD_CAP_RUMBLE_BOOLEAN),
                trigger_rumble: cap(SDL_PROP_GAMEPAD_CAP_TRIGGER_RUMBLE_BOOLEAN),
                light_bar: cap(SDL_PROP_GAMEPAD_CAP_RGB_LED_BOOLEAN),
                player_lights: cap(SDL_PROP_GAMEPAD_CAP_PLAYER_LED_BOOLEAN),
            }
        }
    }

    /// What SDL says about this controller, for a report on it.
    pub fn facts(&self) -> Facts {
        unsafe {
            let js = SDL_GetGamepadJoystick(self.gp);
            let mapping = SDL_GetGamepadMapping(self.gp);
            let props = SDL_GetGamepadProperties(self.gp);
            let caps = [
                (SDL_PROP_GAMEPAD_CAP_RUMBLE_BOOLEAN, "rumble"),
                (SDL_PROP_GAMEPAD_CAP_TRIGGER_RUMBLE_BOOLEAN, "trigger rumble"),
                (SDL_PROP_GAMEPAD_CAP_RGB_LED_BOOLEAN, "light bar"),
                (SDL_PROP_GAMEPAD_CAP_MONO_LED_BOOLEAN, "light"),
                (SDL_PROP_GAMEPAD_CAP_PLAYER_LED_BOOLEAN, "player lights"),
            ];
            let mut f = Facts {
                vendor: self.vendor(),
                product: self.product(),
                version: SDL_GetGamepadProductVersion(self.gp),
                firmware: SDL_GetGamepadFirmwareVersion(self.gp),
                joystick_name: if js.is_null() { String::new() } else { text(SDL_GetJoystickName(js)).unwrap_or_default() },
                gamepad_name: text(SDL_GetGamepadName(self.gp)),
                guid: if js.is_null() { String::new() } else { guid_string(SDL_GetJoystickGUID(js)) },
                path: model_path(&self.path()),
                gamepad_type: gamepad_type(SDL_GetGamepadType(self.gp)),
                real_type: gamepad_type(SDL_GetRealGamepadType(self.gp)),
                joystick_type: if js.is_null() { "unknown".into() } else { joystick_type(SDL_GetJoystickType(js)).into() },
                connection: format!("{:?}", self.connection()).to_lowercase(),
                mapping: text(mapping),
                touchpads: SDL_GetNumGamepadTouchpads(self.gp),
                sensors: [(SDL_SENSOR_GYRO, "gyro"), (SDL_SENSOR_ACCEL, "accelerometer")]
                    .into_iter()
                    .filter(|&(s, _)| SDL_GamepadHasSensor(self.gp, s))
                    .map(|(_, n)| n.to_string())
                    .collect(),
                capabilities: caps
                    .into_iter()
                    .filter(|&(c, _)| SDL_GetBooleanProperty(props, c, false))
                    .map(|(_, n)| n.to_string())
                    .collect(),
                ..Facts::default()
            };
            if !mapping.is_null() {
                SDL_free(mapping.cast());
            }
            if !js.is_null() {
                joystick_counts(js, &mut f);
            }
            f
        }
    }

    /// Reads the current state into `out`. SDL keeps it cached, so this costs no I/O.
    pub fn read(&self, out: &mut PadState) {
        let mut b = 0u64;
        for i in 0..BUTTONS {
            if unsafe { SDL_GetGamepadButton(self.gp, SDL_GamepadButton(i as i32)) } {
                b |= 1 << i;
            }
        }
        b |= self.touch_buttons(b & 1 << button::TOUCHPAD != 0);
        out.buttons = b;
        for i in 0..axis::COUNT {
            out.axes[i] = unsafe { SDL_GetGamepadAxis(self.gp, SDL_GamepadAxis(i as i32)) };
        }
    }

    /// The touchpad's halves and two-finger touch, as the bits OpenController gives them.
    fn touch_buttons(&self, clicked: bool) -> u64 {
        unsafe {
            if SDL_GetNumGamepadTouchpads(self.gp) == 0 {
                return 0;
            }
            let n = SDL_GetNumGamepadTouchpadFingers(self.gp, 0).clamp(0, 4);
            let mut fingers = [Finger::default(); 4];
            for (i, f) in fingers.iter_mut().enumerate().take(n as usize) {
                let mut p = 0f32;
                SDL_GetGamepadTouchpadFinger(self.gp, 0, i as i32, &mut f.down, &mut f.x, &mut f.y, &mut p);
            }
            let (left, right, two) = touch_buttons(clicked, &fingers[..n as usize]);
            u64::from(left) << TOUCH_LEFT | u64::from(right) << TOUCH_RIGHT | u64::from(two) << TOUCH_TWO
        }
    }

    /// Turns the gyro's reports on or off; they cost bandwidth, so only while it is used.
    pub fn set_gyro(&self, on: bool) {
        unsafe { SDL_SetGamepadSensorEnabled(self.gp, SDL_SENSOR_GYRO, on) };
    }

    /// The gyro's last reading in radians per second (pitch, yaw, roll), if it is on.
    pub fn gyro(&self) -> Option<[f32; 3]> {
        let mut v = [0f32; 3];
        unsafe { SDL_GetGamepadSensorData(self.gp, SDL_SENSOR_GYRO, v.as_mut_ptr(), 3) }.then_some(v)
    }

    /// `low` drives the large (left) motor and `high` the small one. SDL stops the rumble after
    /// `ms`, at most 65535, so a steady rumble has to be renewed. Sent from the controller's own
    /// rumble thread; this returns at once.
    pub fn rumble(&self, low: u16, high: u16, ms: u32) {
        self.rumbler.send(low, high, ms);
    }

    /// Lights the player number on controllers that show one (DualSense, Switch) and picks the
    /// matching light bar colour on a DualShock 4. `-1` turns the indicator off.
    pub fn set_player(&self, index: i32) {
        unsafe { SDL_SetGamepadPlayerIndex(self.gp, index) };
    }

    /// Adds fields to the gamepad mapping SDL uses for this controller, for buttons its driver
    /// reports but its mapping leaves out. Applies at once to the open gamepad.
    pub fn extend_mapping(&self, fields: &str) {
        unsafe {
            let current = SDL_GetGamepadMapping(self.gp);
            let Some(mapping) = text(current) else { return };
            SDL_free(current.cast());
            if fields.split(',').all(|f| mapping.split(',').any(|m| m == f)) {
                return;
            }
            let Ok(extended) = std::ffi::CString::new(format!("{},{fields}", mapping.trim_end_matches(','))) else { return };
            SDL_SetGamepadMapping(SDL_GetGamepadID(self.gp), extended.as_ptr());
        }
    }

    /// Colours the light bar; once set, the player number no longer changes it.
    pub fn set_led(&self, [r, g, b]: [u8; 3]) {
        unsafe { SDL_SetGamepadLED(self.gp, r, g, b) };
    }

    pub fn player(&self) -> Option<u8> {
        u8::try_from(unsafe { SDL_GetGamepadPlayerIndex(self.gp) }).ok()
    }
}

impl Drop for Gamepad {
    fn drop(&mut self) {
        unsafe { SDL_CloseGamepad(self.gp) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indices_match_sdl() {
        assert_eq!(button::SOUTH as i32, SDL_GAMEPAD_BUTTON_SOUTH.0);
        assert_eq!(button::GUIDE as i32, SDL_GAMEPAD_BUTTON_GUIDE.0);
        assert_eq!(button::DPAD_RIGHT as i32, SDL_GAMEPAD_BUTTON_DPAD_RIGHT.0);
        assert_eq!(button::MISC1 as i32, SDL_GAMEPAD_BUTTON_MISC1.0);
        assert_eq!(button::TOUCHPAD as i32, SDL_GAMEPAD_BUTTON_TOUCHPAD.0);
        assert_eq!(button::COUNT as i32, SDL_GAMEPAD_BUTTON_COUNT.0);
        assert_eq!(axis::LEFT_Y as i32, SDL_GAMEPAD_AXIS_LEFTY.0);
        assert_eq!(axis::RIGHT_TRIGGER as i32, SDL_GAMEPAD_AXIS_RIGHT_TRIGGER.0);
        assert_eq!(axis::COUNT as i32, SDL_GAMEPAD_AXIS_COUNT.0);
    }

    /// The Linux lines OpenController adds itself: PlayStation controllers read through
    /// `hid-playstation`'s event node, at versions SDL's own database has no line for.
    const LINUX_LINES: [&str; 4] = [
        "030000004c050000e60c000000810000",
        "030000004c050000f20d000000810000",
        "030000004c050000f20d000011810000",
        "050000004c050000f20d000000810000",
    ];

    #[test]
    fn mappings_are_windows_only_but_for_known_linux_lines_and_well_formed() {
        let lines: Vec<&str> = MAPPINGS.lines().filter(|l| !l.starts_with('#') && !l.is_empty()).collect();
        assert!(lines.len() > 500);
        let guid = |l: &str| l.split(',').next().map(|g| g.len() == 32 || g == "xinput").unwrap_or(false);
        assert!(lines.iter().all(|l| guid(l)), "{:?}", lines.iter().find(|l| !guid(l)));
        let linux: Vec<&str> = lines.iter().filter(|l| !l.ends_with("platform:Windows,")).copied().collect();
        assert!(linux.iter().all(|l| l.ends_with("platform:Linux,")), "{linux:?}");
        let ids: Vec<&str> = linux.iter().filter_map(|l| l.split(',').next()).collect();
        assert_eq!(ids, LINUX_LINES);
        // The Xbox layout hid-playstation's buttons and axes come in.
        let layout = "a:b0,b:b1,back:b8,dpdown:h0.4,dpleft:h0.8,dpright:h0.2,dpup:h0.1,guide:b10,leftshoulder:b4,leftstick:b11,\
                      lefttrigger:a2,leftx:a0,lefty:a1,rightshoulder:b5,rightstick:b12,righttrigger:a5,rightx:a3,righty:a4,start:b9,x:b3,y:b2,";
        assert!(linux.iter().all(|l| l.contains(layout)), "{linux:?}");
    }
}
