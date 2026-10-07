use open_controller_core::binding::{Action, Chord, XboxButton};
use open_controller_core::extras::{LEFT_PADDLE1, RIGHT_PADDLE1, TOUCH_LEFT, TOUCHPAD};
use open_controller_core::profile::{Edit, Gyro, GyroMode, Profiles, Sticks};
use open_controller_core::{Command, Config, Engine, PadKey, Role};
use sdl3_sys::everything::*;
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::FILETIME;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_F24};
use windows_sys::Win32::UI::Input::XboxController::{
    XINPUT_GAMEPAD_A, XINPUT_GAMEPAD_X, XINPUT_GAMEPAD_Y, XINPUT_STATE, XINPUT_VIBRATION, XInputGetState, XInputSetState,
};

/// The Xbox set, two back paddles and the touchpad click.
const BUTTON_MASK: u32 = ((1 << 15) - 1) | (1 << RIGHT_PADDLE1) | (1 << LEFT_PADDLE1) | (1 << TOUCHPAD);

/// The virtual controller has no serial, so its assignments are kept by model.
const PROFILE: &str = "model:1209:0c0c";

static RUMBLE: AtomicU32 = AtomicU32::new(0);
static PLAYER: AtomicI32 = AtomicI32::new(-1);

unsafe extern "C" fn on_rumble(_: *mut std::ffi::c_void, low: u16, high: u16) -> bool {
    RUMBLE.store((low as u32) << 16 | high as u32, Ordering::Release);
    true
}

unsafe extern "C" fn on_player(_: *mut std::ffi::c_void, index: i32) {
    PLAYER.store(index, Ordering::Release);
}

fn attach() -> (SDL_JoystickID, *mut SDL_Joystick) {
    attach_one(true)
}

/// A virtual gamepad with a touchpad and a gyro. Only the first one reports rumble and its
/// player number, through the statics.
fn attach_one(first: bool) -> (SDL_JoystickID, *mut SDL_Joystick) {
    // SDL copies these when attaching.
    let touchpads = [SDL_VirtualJoystickTouchpadDesc { nfingers: 2, ..Default::default() }];
    let sensors = [SDL_VirtualJoystickSensorDesc { r#type: SDL_SENSOR_GYRO, rate: 250.0 }];
    let desc = SDL_VirtualJoystickDesc {
        version: size_of::<SDL_VirtualJoystickDesc>() as u32,
        r#type: SDL_JOYSTICK_TYPE_GAMEPAD.0 as u16,
        vendor_id: 0x1209,
        // Virtual joysticks share one device path; a product id of its own keeps the second one
        // from passing for another connection of the first.
        product_id: if first { 0x0C0C } else { 0x0C0D },
        naxes: 6,
        nbuttons: BUTTON_MASK.count_ones() as u16,
        ntouchpads: 1,
        nsensors: 1,
        button_mask: BUTTON_MASK,
        axis_mask: 0x3F,
        name: c"Loopback Controller".as_ptr(),
        touchpads: touchpads.as_ptr(),
        sensors: sensors.as_ptr(),
        Rumble: if first { Some(on_rumble) } else { None },
        SetPlayerIndex: if first { Some(on_player) } else { None },
        ..Default::default()
    };
    let id = unsafe { SDL_AttachVirtualJoystick(&desc) };
    assert!(id.0 != 0, "SDL_AttachVirtualJoystick failed");
    let joy = unsafe { SDL_OpenJoystick(id) };
    assert!(!joy.is_null());
    (id, joy)
}

fn xinput(slot: u32) -> Option<XINPUT_STATE> {
    let mut s: XINPUT_STATE = unsafe { std::mem::zeroed() };
    (unsafe { XInputGetState(slot, &mut s) } == 0).then_some(s)
}

fn wait_for<T>(what: &str, timeout: Duration, mut f: impl FnMut() -> Option<T>) -> T {
    let start = Instant::now();
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(start.elapsed() < timeout, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_micros(200));
    }
}

fn check(ok: bool, what: &str) {
    println!("{} {what}", if ok { "ok  " } else { "FAIL" });
    if !ok {
        std::process::exit(1);
    }
}

fn main() {
    let dir = std::env::temp_dir().join("open-controller-loopback");
    let mut profiles = Profiles::default();
    profiles.apply(Edit::Bind { button: RIGHT_PADDLE1, action: Some(Action::Xbox(XboxButton::Y)) });
    profiles.apply(Edit::Bind { button: LEFT_PADDLE1, action: Some(Action::Keys(Chord { mods: 0, key: VK_F24 })) });
    let config =
        Config { data_dir: dir, hide: false, profiles: [(PROFILE.to_string(), profiles.clone())].into(), driver: Default::default() };
    let engine = Engine::start(config, || {});
    wait_for("the engine", Duration::from_secs(10), || engine.snapshot().running.then_some(()));
    if !matches!(engine.snapshot().vigem, open_controller_core::Driver::Ready { .. }) {
        println!("ViGEmBus is not available: {:?}", engine.snapshot().vigem);
        std::process::exit(1);
    }

    let (id, joy) = attach();
    let slot = wait_for("a virtual Xbox controller", Duration::from_secs(10), || {
        engine.snapshot().pads.iter().find_map(|p| match p.role {
            Role::Virtual { player: Some(i) } => Some(i as u32),
            _ => None,
        })
    });
    check(xinput(slot).is_some(), &format!("XInput slot {slot} is connected"));
    wait_for("the player number", Duration::from_secs(2), || (PLAYER.load(Ordering::Acquire) == slot as i32).then_some(()));
    check(true, &format!("the controller was told it is player {}", slot + 1));

    // The first input after a controller connects is a one-off, measured on its own.
    let t = Instant::now();
    unsafe { SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_LEFTX.0, -7) };
    wait_for("the first input", Duration::from_secs(5), || xinput(slot).filter(|s| s.Gamepad.sThumbLX == -7));
    let first = t.elapsed();

    // End to end: SDL state change to XInput, through the engine's poll loop.
    let mut times = Vec::new();
    for i in 1..=300i16 {
        let value = (i % 2 * 2 - 1) * (1000 + i * 90);
        let t = Instant::now();
        unsafe { SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_LEFTX.0, value) };
        // Spins instead of sleeping between reads: a sleep would add up to 0.5 ms of its own.
        while xinput(slot).is_none_or(|s| s.Gamepad.sThumbLX != value) {
            assert!(t.elapsed() < Duration::from_millis(500), "the stick never arrived");
            std::hint::spin_loop();
        }
        times.push(t.elapsed());
        std::thread::sleep(Duration::from_micros(2000 + (i as u64 * 7919) % 5000));
    }
    times.sort();
    println!(
        "ok   stick moves reach XInput: median {:.2?}, p99 {:.2?}, max {:.2?} (n={}); the first input after connecting {:.2?}",
        times[times.len() / 2],
        times[times.len() * 99 / 100],
        times[times.len() - 1],
        times.len(),
        first
    );
    unsafe { SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_LEFTY.0, -32768) };
    let s = wait_for("the Y axis", Duration::from_millis(500), || xinput(slot).filter(|s| s.Gamepad.sThumbLY == 32767));
    check(s.Gamepad.sThumbLY == 32767, "stick up in SDL is stick up in XInput");
    unsafe { SDL_SetJoystickVirtualButton(joy, SDL_GAMEPAD_BUTTON_SOUTH.0, true) };
    wait_for("button A", Duration::from_millis(500), || xinput(slot).filter(|s| s.Gamepad.wButtons & XINPUT_GAMEPAD_A != 0));
    check(true, "the bottom face button is A");
    unsafe { SDL_SetJoystickVirtualButton(joy, SDL_GAMEPAD_BUTTON_SOUTH.0, false) };

    let pad = engine.snapshot().pads.into_iter().find(|p| p.store.as_deref() == Some(PROFILE));
    check(
        pad.is_some_and(|p| p.extras.contains(&RIGHT_PADDLE1) && p.extras.contains(&LEFT_PADDLE1) && p.bindings().len() == 2),
        "its two paddles are listed with their assignments",
    );
    // The virtual joystick numbers its buttons in the order of the mask: the paddles, gamepad
    // buttons 16 and 17, are its buttons 15 and 16.
    let paddle = |b: u8, down: bool| unsafe {
        SDL_SetJoystickVirtualButton(joy, BUTTON_MASK.count_ones() as i32 - (BUTTON_MASK >> b).count_ones() as i32, down)
    };
    let y = |down: bool| move || xinput(slot).filter(|s| (s.Gamepad.wButtons & XINPUT_GAMEPAD_Y != 0) == down);
    paddle(RIGHT_PADDLE1, true);
    wait_for("the paddle as Y", Duration::from_millis(500), y(true));
    paddle(RIGHT_PADDLE1, false);
    wait_for("Y to be let go", Duration::from_millis(500), y(false));
    check(true, "a paddle assigned to Y presses Y, and lets it go");
    let f24 = |down: bool| move || ((unsafe { GetAsyncKeyState(VK_F24 as i32) } as u16 & 0x8000 != 0) == down).then_some(());
    paddle(LEFT_PADDLE1, true);
    wait_for("F24 down", Duration::from_millis(500), f24(true));
    paddle(LEFT_PADDLE1, false);
    wait_for("F24 up", Duration::from_millis(500), f24(false));
    check(true, "a paddle assigned to a key holds it while held");
    // A second profile, switched to: the paddle becomes RT and the other one does nothing.
    profiles.apply(Edit::Add("Racing".into()));
    profiles.apply(Edit::Bind { button: RIGHT_PADDLE1, action: Some(Action::Xbox(XboxButton::RightTrigger)) });
    profiles.apply(Edit::Bind { button: LEFT_PADDLE1, action: None });
    engine.send(Command::SetProfiles(PROFILE.into(), profiles.clone()));
    paddle(RIGHT_PADDLE1, true);
    wait_for("the paddle as RT", Duration::from_millis(500), || xinput(slot).filter(|s| s.Gamepad.bRightTrigger == 255));
    paddle(RIGHT_PADDLE1, false);
    paddle(LEFT_PADDLE1, true);
    std::thread::sleep(Duration::from_millis(50));
    let f24_after = f24(false)().is_some();
    paddle(LEFT_PADDLE1, false);
    check(f24_after, "switching to another profile takes effect at once, and the old assignments are gone");

    // The virtual joystick numbers its buttons in the order of the mask.
    let button = |b: u8, down: bool| unsafe {
        SDL_SetJoystickVirtualButton(joy, BUTTON_MASK.count_ones() as i32 - (BUTTON_MASK >> b).count_ones() as i32, down)
    };
    let x = |down: bool| move || xinput(slot).filter(|s| (s.Gamepad.wButtons & XINPUT_GAMEPAD_X != 0) == down);
    profiles.apply(Edit::Bind { button: TOUCH_LEFT, action: Some(Action::Xbox(XboxButton::X)) });
    engine.send(Command::SetProfiles(PROFILE.into(), profiles.clone()));
    let pad = wait_for("the touchpad's buttons", Duration::from_secs(1), || {
        engine.snapshot().pads.into_iter().find(|p| p.store.as_deref() == Some(PROFILE) && p.extras.contains(&TOUCH_LEFT))
    });
    check(pad.features.touchpad && pad.features.motion, "a touchpad and a gyro are seen, and the touchpad's halves listed");
    unsafe { SDL_SetJoystickVirtualTouchpad(joy, 0, 0, true, 0.2, 0.5, 1.0) };
    button(TOUCHPAD, true);
    wait_for("a click on the left half as X", Duration::from_millis(500), x(true));
    button(TOUCHPAD, false);
    wait_for("X to be let go", Duration::from_millis(500), x(false));
    unsafe { SDL_SetJoystickVirtualTouchpad(joy, 0, 0, true, 0.8, 0.5, 1.0) };
    button(TOUCHPAD, true);
    std::thread::sleep(Duration::from_millis(50));
    let right_is_not_x = x(false)().is_some();
    button(TOUCHPAD, false);
    unsafe { SDL_SetJoystickVirtualTouchpad(joy, 0, 0, false, 0.8, 0.5, 0.0) };
    check(right_is_not_x, "a click on the left half of the touchpad is a button of its own, the right half another");

    // A profile for a program: in front, its assignments; away, the chosen profile's again.
    profiles.apply(Edit::Select(0));
    profiles.apply(Edit::AddProgram(1, "loopback-game.exe".into()));
    engine.send(Command::SetProfiles(PROFILE.into(), profiles.clone()));
    engine.send(Command::Foreground("loopback-game.exe".into()));
    paddle(RIGHT_PADDLE1, true);
    wait_for("the program's profile (RT)", Duration::from_millis(500), || xinput(slot).filter(|s| s.Gamepad.bRightTrigger == 255));
    paddle(RIGHT_PADDLE1, false);
    engine.send(Command::Foreground("explorer.exe".into()));
    paddle(RIGHT_PADDLE1, true);
    wait_for("the chosen profile again (Y)", Duration::from_millis(500), y(true));
    paddle(RIGHT_PADDLE1, false);
    wait_for("Y to be let go", Duration::from_millis(500), y(false));
    check(true, "a profile that names the program in front takes over, and gives way when it leaves");

    // The gyro: turning left pushes the right stick left.
    let gyro = |yaw: f32, ms: u64| {
        let start = Instant::now();
        let mut t = 0u64;
        while start.elapsed() < Duration::from_millis(ms) {
            t += 4_000_000;
            let v = [0.0f32, yaw, 0.0];
            unsafe { SDL_SendJoystickVirtualSensorData(joy, SDL_SENSOR_GYRO, t, v.as_ptr(), 3) };
            std::thread::sleep(Duration::from_millis(4));
        }
    };
    profiles.apply(Edit::Gyro(Gyro { mode: GyroMode::Always, ..Gyro::default() }));
    engine.send(Command::SetProfiles(PROFILE.into(), profiles.clone()));
    std::thread::sleep(Duration::from_millis(50));
    gyro(2.0, 150);
    let turned = xinput(slot).map(|s| s.Gamepad.sThumbRX).unwrap_or(0);
    gyro(0.0, 300);
    let still = xinput(slot).map(|s| s.Gamepad.sThumbRX).unwrap_or(1);
    check(turned < -20000 && still == 0, &format!("the gyro turns the right stick ({turned}), and a still controller leaves it centred"));
    profiles.apply(Edit::Gyro(Gyro { mode: GyroMode::Aiming, ..Gyro::default() }));
    engine.send(Command::SetProfiles(PROFILE.into(), profiles.clone()));
    std::thread::sleep(Duration::from_millis(50));
    gyro(2.0, 150);
    let not_aiming = xinput(slot).map(|s| s.Gamepad.sThumbRX).unwrap_or(1);
    unsafe { SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_LEFT_TRIGGER.0, 32767) };
    gyro(2.0, 150);
    let aiming = xinput(slot).map(|s| s.Gamepad.sThumbRX).unwrap_or(0);
    unsafe { SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_LEFT_TRIGGER.0, 0) };
    gyro(0.0, 100);
    check(not_aiming == 0 && aiming < -20000, "with \"while aiming\", the gyro only works with the left trigger pulled");
    let (rb, lb) = (SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER.0 as u8, SDL_GAMEPAD_BUTTON_LEFT_SHOULDER.0 as u8);
    profiles.apply(Edit::Gyro(Gyro { mode: GyroMode::Holding(rb), toggle: true, off_button: Some(lb), ..Gyro::default() }));
    engine.send(Command::SetProfiles(PROFILE.into(), profiles.clone()));
    std::thread::sleep(Duration::from_millis(50));
    let press = |b: u8| {
        button(b, true);
        std::thread::sleep(Duration::from_millis(30));
        button(b, false);
        std::thread::sleep(Duration::from_millis(30));
    };
    let rx = || xinput(slot).map(|s| s.Gamepad.sThumbRX).unwrap_or(1);
    gyro(2.0, 150);
    let untoggled = rx();
    press(rb);
    gyro(2.0, 150);
    let toggled = rx();
    button(lb, true);
    gyro(2.0, 150);
    let paused = rx();
    button(lb, false);
    gyro(2.0, 150);
    let resumed = rx();
    press(rb);
    gyro(2.0, 150);
    let toggled_off = rx();
    gyro(0.0, 100);
    check(
        untoggled == 0 && toggled < -20000 && paused == 0 && resumed < -20000 && toggled_off == 0,
        &format!("a toggle turns the gyro on and off ({untoggled}, {toggled}, {toggled_off}), and the off button stops it while held ({paused})"),
    );
    profiles.apply(Edit::Gyro(Gyro::default()));

    // A deadzone for a drifting stick, on the X axis alone (the radial deadzone counts both).
    unsafe { SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_LEFTY.0, 0) };
    profiles.apply(Edit::Sticks(Sticks { deadzone: 20, anti_deadzone: 0 }));
    engine.send(Command::SetProfiles(PROFILE.into(), profiles.clone()));
    unsafe { SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_LEFTX.0, 5000) };
    std::thread::sleep(Duration::from_millis(50));
    let small = xinput(slot).map(|s| s.Gamepad.sThumbLX).unwrap_or(1);
    unsafe { SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_LEFTX.0, 32767) };
    let full = wait_for("the full stick", Duration::from_millis(500), || xinput(slot).filter(|s| s.Gamepad.sThumbLX == 32767));
    unsafe { SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_LEFTX.0, 0) };
    check(small == 0 && full.Gamepad.sThumbLX == 32767, "a 20 % deadzone centres a small movement and keeps the full one");
    profiles.apply(Edit::Sticks(Sticks::default()));
    engine.send(Command::SetProfiles(PROFILE.into(), profiles.clone()));

    // Two controllers trade players.
    let (id2, joy2) = attach_one(false);
    let players = || {
        let mut v: Vec<(PadKey, u8)> = engine
            .snapshot()
            .pads
            .iter()
            .filter_map(|p| match p.role {
                Role::Virtual { player: Some(n) } => Some((p.key, n)),
                _ => None,
            })
            .collect();
        v.sort_by_key(|x| x.1);
        v
    };
    let both = wait_for("a second virtual controller", Duration::from_secs(10), || Some(players()).filter(|v| v.len() == 2));
    let (first_key, second) =
        (both.iter().find(|x| u32::from(x.1) == slot).unwrap().0, both.iter().find(|x| u32::from(x.1) != slot).unwrap());
    let (second_key, second_slot) = (second.0, u32::from(second.1));
    engine.send(Command::SwapPlayers(first_key, second_key));
    wait_for("the swap", Duration::from_secs(2), || {
        players().iter().any(|&(k, n)| k == first_key && u32::from(n) == second_slot).then_some(())
    });
    unsafe { SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_LEFTX.0, 23456) };
    wait_for("the first controller in the second's slot", Duration::from_millis(500), || {
        xinput(second_slot).filter(|s| s.Gamepad.sThumbLX == 23456)
    });
    unsafe { SDL_SetJoystickVirtualAxis(joy2, SDL_GAMEPAD_AXIS_LEFTX.0, -23456) };
    wait_for("the second controller in the first's slot", Duration::from_millis(500), || {
        xinput(slot).filter(|s| s.Gamepad.sThumbLX == -23456)
    });
    check(true, &format!("two controllers trade players {} and {} without either virtual controller leaving", slot + 1, second_slot + 1));
    engine.send(Command::SwapPlayers(first_key, second_key));
    unsafe { SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_LEFTX.0, 0) };
    unsafe { SDL_CloseJoystick(joy2) };
    unsafe { SDL_DetachVirtualJoystick(id2) };
    wait_for("the swap back", Duration::from_secs(2), || {
        engine.snapshot().pads.iter().any(|p| p.key == first_key && p.role == Role::Virtual { player: Some(slot as u8) }).then_some(())
    });

    let v = XINPUT_VIBRATION { wLeftMotorSpeed: 0xA000, wRightMotorSpeed: 0x4000 };
    unsafe { XInputSetState(slot, &v) };
    let r = wait_for("rumble", Duration::from_secs(2), || Some(RUMBLE.load(Ordering::Acquire)).filter(|&r| r != 0));
    check(
        (r >> 16) == 0xA0A0 && (r & 0xFFFF) == 0x4040,
        &format!("game rumble reaches the controller (low {:#06x}, high {:#06x})", r >> 16, r & 0xFFFF),
    );
    let off = XINPUT_VIBRATION { wLeftMotorSpeed: 0, wRightMotorSpeed: 0 };
    unsafe { XInputSetState(slot, &off) };

    // The handoff: the controller goes away and comes back as a new device.
    unsafe { SDL_CloseJoystick(joy) };
    let first = engine.snapshot().pads.len();
    unsafe { SDL_DetachVirtualJoystick(id) };
    wait_for("the controller to be away", Duration::from_secs(2), || {
        engine.snapshot().pads.iter().any(|p| matches!(p.role, Role::Waiting { .. })).then_some(())
    });
    let held = xinput(slot);
    check(held.is_some_and(|s| s.Gamepad.sThumbLX == 0 && s.Gamepad.wButtons == 0), "while it is away, the slot stays, at rest");
    let (_, joy) = attach();
    wait_for("the controller to rejoin", Duration::from_secs(2), || {
        engine.snapshot().pads.iter().any(|p| p.role == Role::Virtual { player: Some(slot as u8) }).then_some(())
    });
    check(engine.snapshot().pads.len() == first, "it came back to the same slot, not a new one");
    unsafe { SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_RIGHTX.0, 12345) };
    wait_for("input after the handoff", Duration::from_millis(500), || xinput(slot).filter(|s| s.Gamepad.sThumbRX == 12345));
    check(true, "input flows again after the handoff");

    // What the 1 kHz loop costs with a controller connected and nobody touching it.
    let cpu = || {
        let (mut c, mut e, mut k, mut u) = unsafe { std::mem::zeroed::<[FILETIME; 4]>() }.into();
        unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) };
        let t = |f: FILETIME| ((f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64) as f64 / 1e7;
        t(k) + t(u)
    };
    let (c0, t0) = (cpu(), Instant::now());
    std::thread::sleep(Duration::from_secs(10));
    println!("ok   idle with a controller connected: {:.2} % of one core", (cpu() - c0) / t0.elapsed().as_secs_f64() * 100.0);

    engine.stop();
    check(xinput(slot).is_none(), "stopping the engine unplugs the virtual controller");
}
