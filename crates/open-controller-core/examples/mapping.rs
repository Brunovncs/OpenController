//! An end-to-end check of a mapping made by a joystick's owner, without hardware. SDL hosts a
//! virtual joystick of no known kind in the same process, which SDL has no layout for, and:
//!
//! - it shows as unmapped, with what it sends by its own numbering;
//! - given a mapping line, it becomes a gamepad at once, its buttons, sticks and hat coming
//!   through the mapping;
//! - with the mapping taken away it is unmapped again, though SDL cannot forget a mapping;
//! - given another mapping, it is a gamepad again, read the new way;
//! - a mapping given at the start maps it as soon as it connects.
//!
//! Changes nothing permanent; HidHide is left alone.
//!
//!     cargo run -p open-controller-core --example mapping

use open_controller_core::mapping::{axis, button};
use open_controller_core::{Command, Config, Engine, PadKey, PadView, Role, learn};
use sdl3_sys::everything::*;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const NAME: &std::ffi::CStr = c"Mapping Check Joystick";

fn attach() -> (SDL_JoystickID, *mut SDL_Joystick) {
    let desc = SDL_VirtualJoystickDesc {
        version: size_of::<SDL_VirtualJoystickDesc>() as u32,
        // Of no known kind: SDL makes no layout for it, as for a joystick it does not know.
        r#type: SDL_JOYSTICK_TYPE_UNKNOWN.0 as u16,
        vendor_id: 0x1209,
        product_id: 0x0C0E,
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
    (id, joy)
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
    engine.snapshot().pads.into_iter().filter(|p| (p.vendor, p.product) == (0x1209, 0x0C0E)).collect()
}

fn unmapped(engine: &Engine) -> Option<PadView> {
    ours(engine).into_iter().find(|p| p.role == Role::Unmapped && matches!(p.key, PadKey::Device(_)))
}

fn mapped(engine: &Engine) -> Option<PadView> {
    ours(engine).into_iter().find(|p| matches!(p.key, PadKey::Slot(_)) && p.own_mapping)
}

fn button(joy: *mut SDL_Joystick, b: i32, down: bool) {
    unsafe { SDL_SetJoystickVirtualButton(joy, b, down) };
}

fn config(mappings: BTreeMap<String, String>) -> Config {
    Config {
        data_dir: std::env::temp_dir().join("open-controller-mapping"),
        hide: false,
        profiles: Default::default(),
        driver: Default::default(),
        split_joycons: false,
        mappings,
    }
}

fn start(mappings: BTreeMap<String, String>) -> Engine {
    let engine = Engine::start(config(mappings), || {});
    wait_for("the engine", || engine.snapshot().running.then_some(()));
    // Live input in snapshots, as when the window looks.
    engine.send(Command::Watch(true));
    engine
}

fn main() {
    let engine = start(BTreeMap::new());
    let (_, joy) = attach();

    let pad = wait_for("the joystick, unmapped", || unmapped(&engine));
    let guid = pad.guid.clone();
    check(guid.len() == 32, &format!("it shows as unmapped, GUID {guid}"));
    button(joy, 2, true);
    wait_for("button 2, raw", || unmapped(&engine)?.raw.filter(|r| r.buttons == 1 << 2));
    button(joy, 2, false);
    check(true, "what it sends arrives by its own numbering");

    let line = learn::line(&guid, "Mapping Check Joystick", "a:b2,b:b1,x:b3,y:b0,leftx:a0,lefty:a1,dpup:h0.1,").unwrap();
    engine.send(Command::SetMappings([(guid.clone(), line)].into()));
    let pad = wait_for("a gamepad from the mapping", || mapped(&engine));
    check(unmapped(&engine).is_none(), "mapped, it is a gamepad and no longer unmapped");
    if matches!(engine.snapshot().vigem, open_controller_core::Driver::Ready { .. }) {
        let role = wait_for("a virtual Xbox controller", || {
            ours(&engine).into_iter().find(|p| p.key == pad.key && matches!(p.role, Role::Virtual { .. })).map(|p| p.role)
        });
        check(true, &format!("games see it as an Xbox controller ({role:?})"));
    }
    let slot = pad.key;
    let input =
        |f: &dyn Fn(&PadView) -> bool| wait_for("input through the mapping", || ours(&engine).into_iter().find(|p| p.key == slot && f(p)));
    button(joy, 2, true);
    input(&|p| p.input.pressed(button::SOUTH));
    button(joy, 2, false);
    unsafe { SDL_SetJoystickVirtualAxis(joy, 0, 20000) };
    input(&|p| p.input.axes[axis::LEFT_X] == 20000);
    unsafe { SDL_SetJoystickVirtualHat(joy, 0, SDL_HAT_UP) };
    input(&|p| p.input.pressed(button::DPAD_UP) && !p.input.pressed(button::SOUTH));
    unsafe {
        SDL_SetJoystickVirtualAxis(joy, 0, 0);
        SDL_SetJoystickVirtualHat(joy, 0, SDL_HAT_CENTERED);
    }
    check(true, "its button, stick and hat come through the mapping");

    engine.send(Command::SetMappings(BTreeMap::new()));
    wait_for("the joystick, unmapped again", || unmapped(&engine));
    check(!ours(&engine).iter().any(|p| p.own_mapping), "with the mapping taken away it is unmapped again");
    button(joy, 1, true);
    wait_for("button 1, raw", || unmapped(&engine)?.raw.filter(|r| r.buttons == 1 << 1));
    button(joy, 1, false);
    check(true, "and what it sends arrives raw again");

    let line = learn::line(&guid, "Mapping Check Joystick", "a:b1,b:b2,").unwrap();
    engine.send(Command::SetMappings([(guid.clone(), line.clone())].into()));
    let pad = wait_for("a gamepad from the new mapping", || mapped(&engine));
    check(unmapped(&engine).is_none(), "mapped again, it is a gamepad again");
    let slot = pad.key;
    button(joy, 1, true);
    wait_for("button 1 as A", || ours(&engine).into_iter().find(|p| p.key == slot && p.input.pressed(button::SOUTH)));
    button(joy, 1, false);
    check(true, "read the new way");

    engine.stop();
    let engine = start([(guid.clone(), line)].into());
    let (_, joy) = attach();
    let pad = wait_for("a gamepad as it connects", || mapped(&engine));
    button(joy, 1, true);
    wait_for("button 1 as A", || ours(&engine).into_iter().find(|p| p.key == pad.key && p.input.pressed(button::SOUTH)));
    button(joy, 1, false);
    check(unmapped(&engine).is_none(), "a mapping given at the start maps it as soon as it connects");
    engine.stop();
}
