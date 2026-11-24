//! An end-to-end check, without hardware, of a PlayStation 2 adapter's port (2563:0555, the
//! ShanWan Twin USB Joystick) whose layout comes from SDL's list of controllers:
//!
//! - the port waits for a press before getting a virtual controller, so an empty port never
//!   takes player 1 from the one in use;
//! - pressed, it gets its virtual controller;
//! - the layout from SDL's list can be set aside, the port becoming a joystick to map;
//! - a mapping made by the owner then replaces it.
//!
//! SDL hosts a virtual joystick in the same process (two of them would share one path, and so
//! one slot, which a real adapter's ports do not). Changes nothing permanent; HidHide is left
//! alone. Plugs a virtual Xbox controller while it runs when ViGEmBus is there.
//!
//!     cargo run -p open-controller-core --example adapter_port

use open_controller_core::mapping::button;
use open_controller_core::{Command, Config, Engine, PadKey, PadView, Role, learn};
use sdl3_sys::everything::*;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::time::{Duration, Instant};

const NAME: &std::ffi::CStr = c"Twin USB Joystick";

fn attach() -> *mut SDL_Joystick {
    let desc = SDL_VirtualJoystickDesc {
        version: size_of::<SDL_VirtualJoystickDesc>() as u32,
        r#type: SDL_JOYSTICK_TYPE_UNKNOWN.0 as u16,
        vendor_id: 0x2563,
        product_id: 0x0555,
        naxes: 4,
        nbuttons: 12,
        nhats: 1,
        name: NAME.as_ptr(),
        ..Default::default()
    };
    let id = unsafe { SDL_AttachVirtualJoystick(&desc) };
    assert!(id.0 != 0, "SDL_AttachVirtualJoystick failed");
    let joy = unsafe { SDL_OpenJoystick(id) };
    assert!(!joy.is_null());
    joy
}

fn wait_for<T>(what: &str, mut f: impl FnMut() -> Option<T>) -> T {
    let start = Instant::now();
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn check(ok: bool, what: &str) {
    println!("{} {what}", if ok { "ok  " } else { "FAIL" });
    if !ok {
        std::process::exit(1);
    }
}

fn ours(engine: &Engine) -> Vec<PadView> {
    engine.snapshot().pads.into_iter().filter(|p| (p.vendor, p.product) == (0x2563, 0x0555)).collect()
}

fn slot(engine: &Engine) -> Option<PadView> {
    ours(engine).into_iter().find(|p| matches!(p.key, PadKey::Slot(_)))
}

fn main() {
    let engine = Engine::start(
        Config {
            data_dir: std::env::temp_dir().join("open-controller-adapter-port"),
            hide: false,
            profiles: Default::default(),
            driver: Default::default(),
            split_joycons: false,
            mappings: BTreeMap::new(),
        },
        || {},
    );
    wait_for("the engine", || engine.snapshot().running.then_some(()));
    engine.send(Command::Watch(true));
    let vigem = matches!(engine.snapshot().vigem, open_controller_core::Driver::Ready { .. });

    let port = attach();
    let guid = wait_for("the port, unmapped", || ours(&engine).into_iter().find(|p| p.role == Role::Unmapped).map(|p| p.guid));
    // As a line of SDL's list would: SDL itself is given the layout, not the engine.
    let line = CString::new(learn::line(&guid, "Twin USB Joystick", "a:b2,b:b1,x:b3,y:b0,leftx:a0,lefty:a1,rightx:a2,righty:a3,").unwrap())
        .unwrap();
    check(unsafe { SDL_AddGamepadMapping(line.as_ptr()) } >= 0, "SDL's list gives it a layout");

    wait_for("its slot", || slot(&engine));
    // Long enough for a virtual controller to be plugged, were one asked for.
    std::thread::sleep(Duration::from_millis(1500));
    let idle = slot(&engine).unwrap();
    check(idle.role == Role::Idle, &format!("untouched, it waits for a press with no player ({:?})", idle.role));
    check(!idle.own_mapping, "the layout is not one the owner made");

    unsafe { SDL_SetJoystickVirtualButton(port, 2, true) };
    wait_for("its input", || slot(&engine).filter(|p| p.input.pressed(button::SOUTH)));
    unsafe { SDL_SetJoystickVirtualButton(port, 2, false) };
    if vigem {
        let role = wait_for("a virtual controller", || slot(&engine).filter(|p| matches!(p.role, Role::Virtual { .. })).map(|p| p.role));
        check(true, &format!("pressed, it gets its virtual controller ({role:?})"));
    } else {
        let role = wait_for("a role past waiting", || slot(&engine).filter(|p| p.role != Role::Idle).map(|p| p.role));
        check(true, &format!("pressed, it no longer waits ({role:?}; no ViGEmBus here)"));
    }

    engine.send(Command::Forget(guid.clone()));
    wait_for("the port unmapped", || ours(&engine).into_iter().find(|p| p.role == Role::Unmapped));
    // Its virtual controller is kept a while, as for any controller that goes away.
    let kept = slot(&engine).map(|p| p.role);
    check(
        kept.as_ref().is_none_or(|r| matches!(r, Role::Waiting { .. })),
        &format!("set aside, SDL's layout is gone: it is a joystick to map (slot {kept:?})"),
    );

    let mine = learn::line(&guid, "Twin USB Joystick", "a:b1,b:b2,leftx:a0,lefty:a1,rightx:a3,righty:a2,").unwrap();
    engine.send(Command::SetMappings([(guid.clone(), mine)].into()));
    let pad = wait_for("a gamepad from the owner's mapping", || slot(&engine).filter(|p| p.own_mapping));
    unsafe { SDL_SetJoystickVirtualButton(port, 1, true) };
    wait_for("button 1 as A", || slot(&engine).filter(|p| p.key == pad.key && p.input.pressed(button::SOUTH)));
    unsafe { SDL_SetJoystickVirtualButton(port, 1, false) };
    check(true, "the owner's mapping replaces SDL's layout");
    engine.stop();
}
