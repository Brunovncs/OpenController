//! An end-to-end check of the engine without a physical controller. SDL can host virtual
//! joysticks in the same process; this one plays the part of a controller, the engine turns it
//! into a virtual Xbox 360 controller, and XInput, which is what games read, is checked:
//!
//! - the controller gets an XInput slot, and the slot's player number comes back to it;
//! - a stick move and a button press arrive, and how long that takes end to end;
//! - rumble a game sets through XInput reaches the controller;
//! - the controller disconnecting and reconnecting keeps the same XInput slot (the handoff);
//! - stopping the engine unplugs the virtual controller.
//!
//! Needs ViGEmBus. Changes nothing permanent; HidHide is left alone.
//!
//!     cargo run -p open-controller-core --release --example loopback

use open_controller_core::{Config, Engine, Role};
use sdl3_sys::everything::*;
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::FILETIME;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
use windows_sys::Win32::UI::Input::XboxController::{XINPUT_GAMEPAD_A, XINPUT_STATE, XINPUT_VIBRATION, XInputGetState, XInputSetState};

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
    let desc = SDL_VirtualJoystickDesc {
        version: size_of::<SDL_VirtualJoystickDesc>() as u32,
        r#type: SDL_JOYSTICK_TYPE_GAMEPAD.0 as u16,
        vendor_id: 0x1209,
        product_id: 0x0C0C,
        naxes: 6,
        nbuttons: 15,
        button_mask: (1 << 15) - 1,
        axis_mask: 0x3F,
        name: c"Loopback Controller".as_ptr(),
        Rumble: Some(on_rumble),
        SetPlayerIndex: Some(on_player),
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
    let engine = Engine::start(Config { data_dir: dir, hide: false }, || {});
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
