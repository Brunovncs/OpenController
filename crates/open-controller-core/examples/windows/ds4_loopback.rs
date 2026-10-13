use open_controller_core::devnode;
use open_controller_core::ds4::{Calibration, STANDARD_GRAVITY, VIGEM_CALIBRATION};
use open_controller_core::profile::{DpadStick, Edit, Output, Profiles, Sticks};
use open_controller_core::{Config, Engine, Role};
use sdl3_sys::everything::*;
use std::ffi::CStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

/// The virtual controller has no serial, so its settings are kept by model.
const PROFILE: &str = "model:1209:0c0e";

static RUMBLE: AtomicU32 = AtomicU32::new(0);

unsafe extern "C" fn on_rumble(_: *mut std::ffi::c_void, low: u16, high: u16) -> bool {
    RUMBLE.store((low as u32) << 16 | high as u32, Ordering::Release);
    true
}

/// A virtual gamepad with a gyro and an accelerometer.
fn attach() -> *mut SDL_Joystick {
    let sensors = [
        SDL_VirtualJoystickSensorDesc { r#type: SDL_SENSOR_GYRO, rate: 250.0 },
        SDL_VirtualJoystickSensorDesc { r#type: SDL_SENSOR_ACCEL, rate: 250.0 },
    ];
    let desc = SDL_VirtualJoystickDesc {
        version: size_of::<SDL_VirtualJoystickDesc>() as u32,
        r#type: SDL_JOYSTICK_TYPE_GAMEPAD.0 as u16,
        vendor_id: 0x1209,
        product_id: 0x0C0E,
        naxes: 6,
        nbuttons: 15,
        nsensors: 2,
        button_mask: (1 << 15) - 1,
        axis_mask: 0x3F,
        name: c"DS4 Loopback Controller".as_ptr(),
        sensors: sensors.as_ptr(),
        Rumble: Some(on_rumble),
        ..Default::default()
    };
    let id = unsafe { SDL_AttachVirtualJoystick(&desc) };
    assert!(id.0 != 0, "SDL_AttachVirtualJoystick failed");
    let joy = unsafe { SDL_OpenJoystick(id) };
    assert!(!joy.is_null());
    joy
}

fn wait_for<T>(what: &str, timeout: Duration, mut f: impl FnMut() -> Option<T>) -> T {
    let start = Instant::now();
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(start.elapsed() < timeout, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn check(ok: bool, what: &str) {
    println!("{} {what}", if ok { "ok  " } else { "FAIL" });
    if !ok {
        std::process::exit(1);
    }
}

/// The virtual DualShock 4 ViGEmBus made, opened as a game opens it.
fn open_ds4() -> Option<*mut SDL_hid_device> {
    unsafe {
        let list = SDL_hid_enumerate(0x054C, 0x05C4);
        let mut info = list;
        let mut dev = None;
        while !info.is_null() && dev.is_none() {
            let i = &*info;
            if !i.path.is_null() && devnode::is_virtual(&CStr::from_ptr(i.path).to_string_lossy()) {
                let d = SDL_hid_open_path(i.path);
                dev = (!d.is_null()).then_some(d);
            }
            info = i.next;
        }
        SDL_hid_free_enumeration(list);
        dev
    }
}

/// The DualShock 4's latest input report (id 1), waiting for one that `ok` accepts.
fn report(dev: *mut SDL_hid_device, what: &str, ok: impl Fn(&[u8; 64]) -> bool) -> [u8; 64] {
    let start = Instant::now();
    loop {
        let mut b = [0u8; 64];
        let n = unsafe { SDL_hid_read_timeout(dev, b.as_mut_ptr(), b.len(), 50) };
        if n >= 30 && b[0] == 0x01 && ok(&b) {
            return b;
        }
        assert!(start.elapsed() < Duration::from_secs(3), "timed out waiting for {what}: {:02x?}", &b[..32]);
    }
}

fn word(b: &[u8; 64], at: usize) -> i16 {
    i16::from_le_bytes([b[at], b[at + 1]])
}

/// The gyro (rad/s) and accelerometer (m/s²) in a report, read with the calibration the virtual
/// DualShock 4 gives, as SDL reads them.
fn motion(b: &[u8; 64]) -> ([f32; 3], [f32; 3]) {
    let c = Calibration::from_usb_report(&VIGEM_CALIBRATION);
    let gyro = std::array::from_fn(|i| ((f32::from(word(b, 13 + 2 * i)) - c.gyro[i].0) / c.gyro[i].1).to_radians());
    let accel = std::array::from_fn(|i| (f32::from(word(b, 19 + 2 * i)) - c.accel[i].0) / c.accel[i].1 * STANDARD_GRAVITY);
    (gyro, accel)
}

fn close(a: [f32; 3], b: [f32; 3], tolerance: f32) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < tolerance)
}

fn main() {
    unsafe { SDL_hid_init() };
    let dir = std::env::temp_dir().join("open-controller-ds4-loopback");
    let mut profiles = Profiles::default();
    let config = Config { data_dir: dir, hide: false, profiles: Default::default(), driver: Default::default() };
    let engine = Engine::start(config, || {});
    wait_for("the engine", Duration::from_secs(10), || engine.snapshot().running.then_some(()));
    if !matches!(engine.snapshot().vigem, open_controller_core::Driver::Ready { .. }) {
        println!("ViGEmBus is not available: {:?}", engine.snapshot().vigem);
        std::process::exit(1);
    }

    // An Xbox controller first, as every controller starts.
    let joy = attach();
    let slot = wait_for("a virtual Xbox controller", Duration::from_secs(10), || {
        engine.snapshot().pads.iter().find(|p| p.store.as_deref() == Some(PROFILE)).and_then(|p| match p.role {
            Role::Virtual { player: Some(i) } => Some(u32::from(i)),
            _ => None,
        })
    });
    check(xinput(slot) && open_ds4().is_none(), &format!("it starts as an Xbox controller, in XInput slot {slot}"));
    profiles.apply(Edit::Output(Output::DualShock4));
    engine.send(open_controller_core::Command::SetProfiles(PROFILE.into(), profiles.clone()));
    let dev = wait_for("the virtual DualShock 4", Duration::from_secs(10), open_ds4);
    wait_for("the Xbox controller to leave", Duration::from_secs(5), || (!xinput(slot)).then_some(()));
    check(true, "set to show itself as a DualShock 4, the Xbox controller leaves and a DualShock 4 (054C:05C4) takes its place");
    check(
        engine.snapshot().pads.iter().filter(|p| p.store.as_deref() == Some(PROFILE)).count() == 1,
        "the engine does not take its own DualShock 4 for a new controller",
    );

    unsafe {
        SDL_SetJoystickVirtualButton(joy, SDL_GAMEPAD_BUTTON_SOUTH.0, true);
        SDL_SetJoystickVirtualButton(joy, SDL_GAMEPAD_BUTTON_START.0, true);
        SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_LEFTX.0, i16::MIN);
        SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_RIGHT_TRIGGER.0, i16::MAX);
    }
    let b = report(dev, "Cross, Options, the stick and R2", |b| b[5] & 0x20 != 0 && b[9] == 255);
    check(b[5] & 0x0F == 8 && b[6] & 0x20 != 0 && b[6] & 0x08 != 0, "the bottom face button is Cross, Start is Options and R2 is pressed");
    check(b[1] == 0 && b[3] == 128, &format!("the left stick is all the way left ({}), the right one at rest ({})", b[1], b[3]));
    unsafe {
        SDL_SetJoystickVirtualButton(joy, SDL_GAMEPAD_BUTTON_SOUTH.0, false);
        SDL_SetJoystickVirtualButton(joy, SDL_GAMEPAD_BUTTON_START.0, false);
        SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_LEFTX.0, 0);
        SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_RIGHT_TRIGGER.0, 0);
    }

    // Tilted and turning: the DualShock 4 carries both to games.
    let gyro = [0.5f32, -1.25, 0.75];
    let accel = [1.5f32, 9.0, -3.0];
    let start = Instant::now();
    let mut t = 0u64;
    let mut seen = None;
    while start.elapsed() < Duration::from_secs(2) && seen.is_none() {
        t += 4_000_000;
        unsafe {
            SDL_SendJoystickVirtualSensorData(joy, SDL_SENSOR_GYRO, t, gyro.as_ptr(), 3);
            SDL_SendJoystickVirtualSensorData(joy, SDL_SENSOR_ACCEL, t, accel.as_ptr(), 3);
        }
        let mut b = [0u8; 64];
        if unsafe { SDL_hid_read_timeout(dev, b.as_mut_ptr(), b.len(), 4) } >= 30 {
            let (g, a) = motion(&b);
            if close(g, gyro, 0.01) && close(a, accel, 0.02) {
                seen = Some((g, a));
            }
        }
    }
    check(seen.is_some(), &format!("the gyro and the accelerometer reach games as SDL reads them back ({seen:?})"));

    // The right stick standing in for the d-pad.
    profiles.apply(Edit::Sticks(Sticks { dpad: DpadStick::Right, ..Sticks::default() }));
    engine.send(open_controller_core::Command::SetProfiles(PROFILE.into(), profiles.clone()));
    std::thread::sleep(Duration::from_millis(50));
    unsafe { SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_RIGHTY.0, i16::MIN) };
    let b = report(dev, "the d-pad up", |b| b[5] & 0x0F == 0);
    check(b[3] == 128 && b[4] == 128, "the right stick pressed up is the d-pad up, and the stick stays at rest");
    unsafe { SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_RIGHTY.0, 0) };

    // A game rumbling the DualShock 4: its output report, motors at bytes 4 (small) and 5 (large).
    let mut out = [0u8; 32];
    out[0] = 0x05;
    out[1] = 0x07;
    out[4] = 0x40;
    out[5] = 0xC0;
    let wrote = unsafe { SDL_hid_write(dev, out.as_ptr(), out.len()) };
    check(wrote > 0, "a game's rumble request is taken");
    let r = wait_for("the rumble", Duration::from_secs(2), || Some(RUMBLE.load(Ordering::Acquire)).filter(|&r| r != 0));
    check(r >> 16 == 0xC0 * 257 && r & 0xFFFF == 0x40 * 257, &format!("it reaches the controller, large and small motor ({r:#010x})"));

    unsafe { SDL_hid_close(dev) };
    profiles.apply(Edit::Output(Output::Xbox360));
    engine.send(open_controller_core::Command::SetProfiles(PROFILE.into(), profiles.clone()));
    wait_for("an Xbox controller again", Duration::from_secs(10), || (0..4).any(xinput).then_some(()));
    wait_for("the DualShock 4 to leave", Duration::from_secs(5), || open_ds4().is_none().then_some(()));
    check(true, "set back, it is an Xbox controller again");
    profiles.apply(Edit::Output(Output::DualShock4));
    engine.send(open_controller_core::Command::SetProfiles(PROFILE.into(), profiles.clone()));
    wait_for("the DualShock 4 again", Duration::from_secs(10), || open_ds4().map(|d| unsafe { SDL_hid_close(d) }));
    engine.stop();
    std::thread::sleep(Duration::from_millis(500));
    check(open_ds4().is_none(), "stopping the engine unplugs the DualShock 4");
}

fn xinput(slot: u32) -> bool {
    use windows_sys::Win32::UI::Input::XboxController::{XINPUT_STATE, XInputGetState};
    let mut s: XINPUT_STATE = unsafe { std::mem::zeroed() };
    unsafe { XInputGetState(slot, &mut s) == 0 }
}
