//! A thin wrapper over the parts of SDL 3 that read gamepads.
//!
//! SDL does the hard work of talking to each controller in its own protocol: the DualShock 4
//! and DualSense over USB, Bluetooth and the Sony receiver, Switch Pro and Joy-Con, Xbox pads
//! over XInput, and hundreds of generic pads through the bundled mapping database. Everything
//! here must run on the one thread that called [`Sdl::init`]: SDL reports hot-plugging only to
//! that thread, and keeping all calls there avoids contention on its joystick lock.

use crate::device::{PadType, Power, SdlConnection};
use crate::extras::{Features, TOUCH_LEFT, TOUCH_RIGHT, TOUCH_TWO};
use crate::mapping::{PadState, axis, button};
use crate::motion::{Finger, touch_buttons};
use sdl3_sys::everything::*;
use std::ffi::{CStr, c_char};
use std::marker::PhantomData;

static MAPPINGS: &str = include_str!("../assets/gamecontrollerdb.txt");

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

    pub fn open(&self, id: u32) -> Option<Gamepad> {
        let gp = unsafe { SDL_OpenGamepad(SDL_JoystickID(id)) };
        (!gp.is_null()).then_some(Gamepad { gp })
    }
}

impl Drop for Sdl {
    fn drop(&mut self) {
        unsafe { SDL_Quit() };
    }
}

pub struct Gamepad {
    gp: *mut SDL_Gamepad,
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
    /// `ms`, at most 65535, so a steady rumble has to be renewed.
    pub fn rumble(&self, low: u16, high: u16, ms: u32) {
        unsafe { SDL_RumbleGamepad(self.gp, low, high, ms) };
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

    #[test]
    fn mappings_are_windows_only_and_well_formed() {
        let lines: Vec<&str> = MAPPINGS.lines().filter(|l| !l.starts_with('#') && !l.is_empty()).collect();
        assert!(lines.len() > 500);
        let guid = |l: &str| l.split(',').next().map(|g| g.len() == 32 || g == "xinput").unwrap_or(false);
        assert!(lines.iter().all(|l| l.ends_with("platform:Windows,") && guid(l)), "{:?}", lines.iter().find(|l| !guid(l)));
    }
}
