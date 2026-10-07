use open_controller_core::binding::{Action, Chord, XboxButton};
use open_controller_core::extras::{LEFT_PADDLE1, RIGHT_PADDLE1};
use open_controller_core::profile::{Edit, Profiles};
use open_controller_core::{Config, Engine, Role};
use sdl3_sys::everything::*;
use std::fs::File;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

/// The Xbox set and two back paddles.
const BUTTON_MASK: u32 = ((1 << 15) - 1) | (1 << RIGHT_PADDLE1) | (1 << LEFT_PADDLE1);
const PROFILE: &str = "model:1209:0c0c";
const VK_F24: u16 = 0x87;

const BTN_A: usize = 0x130;
const BTN_Y: usize = 0x134;
const KEY_F24: usize = 194;
const ABS_X: u32 = 0;
const ABS_Y: u32 = 1;

static RUMBLE: AtomicU32 = AtomicU32::new(0);

unsafe extern "C" fn on_rumble(_: *mut std::ffi::c_void, low: u16, high: u16) -> bool {
    RUMBLE.store((low as u32) << 16 | high as u32, Ordering::Release);
    true
}

const fn ioc(dir: u32, nr: u32, size: usize) -> libc::c_ulong {
    ((dir << 30) | ((size as u32) << 16) | ((b'E' as u32) << 8) | nr) as libc::c_ulong
}

fn attach() -> *mut SDL_Joystick {
    let desc = SDL_VirtualJoystickDesc {
        version: size_of::<SDL_VirtualJoystickDesc>() as u32,
        r#type: SDL_JOYSTICK_TYPE_GAMEPAD.0 as u16,
        vendor_id: 0x1209,
        product_id: 0x0C0C,
        naxes: 6,
        nbuttons: BUTTON_MASK.count_ones() as u16,
        button_mask: BUTTON_MASK,
        axis_mask: 0x3F,
        name: c"Loopback Controller".as_ptr(),
        Rumble: Some(on_rumble),
        ..Default::default()
    };
    let id = unsafe { SDL_AttachVirtualJoystick(&desc) };
    assert!(id.0 != 0, "SDL_AttachVirtualJoystick failed");
    let joy = unsafe { SDL_OpenJoystick(id) };
    assert!(!joy.is_null());
    joy
}

/// The event node of the input device whose `phys` or name matches.
fn node(matches: impl Fn(&str, &str) -> bool) -> Option<File> {
    for e in std::fs::read_dir("/sys/class/input").ok()?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.starts_with("event") {
            continue;
        }
        let read = |f: &str| std::fs::read_to_string(e.path().join("device").join(f)).unwrap_or_default().trim().to_string();
        if matches(&read("phys"), &read("name")) {
            return File::options().read(true).write(true).open(format!("/dev/input/{name}")).ok();
        }
    }
    None
}

fn abs(f: &File, axis: u32) -> i32 {
    let mut info: libc::input_absinfo = unsafe { std::mem::zeroed() };
    unsafe { libc::ioctl(f.as_raw_fd(), ioc(2, 0x40 + axis, size_of::<libc::input_absinfo>()), &mut info) };
    info.value
}

fn key(f: &File, code: usize) -> bool {
    let mut bits = [0u8; 96];
    unsafe { libc::ioctl(f.as_raw_fd(), ioc(2, 0x18, bits.len()), bits.as_mut_ptr()) };
    bits[code / 8] & (1 << (code % 8)) != 0
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
    let mut profiles = Profiles::default();
    profiles.apply(Edit::Bind { button: RIGHT_PADDLE1, action: Some(Action::Xbox(XboxButton::Y)) });
    profiles.apply(Edit::Bind { button: LEFT_PADDLE1, action: Some(Action::Keys(Chord { mods: 0, key: VK_F24 })) });
    let dir = std::env::temp_dir().join("open-controller-loopback");
    let config = Config { data_dir: dir, hide: false, profiles: [(PROFILE.to_string(), profiles)].into(), driver: Default::default() };
    let engine = Engine::start(config, || {});
    wait_for("the engine", Duration::from_secs(10), || engine.snapshot().running.then_some(()));
    if !matches!(engine.snapshot().vigem, open_controller_core::Driver::Ready { .. }) {
        println!("uinput is not available: {:?}", engine.snapshot().vigem);
        std::process::exit(1);
    }

    let joy = attach();
    let player = wait_for("a virtual Xbox controller", Duration::from_secs(10), || {
        engine.snapshot().pads.iter().find_map(|p| match p.role {
            Role::Virtual { player: Some(i) } => Some(i),
            _ => None,
        })
    });
    check(player == 0, "the controller is player 1");
    let pad = wait_for("its event node", Duration::from_secs(5), || node(|phys, _| phys.starts_with("open-controller/")));
    let name = std::fs::read_to_string("/proc/bus/input/devices").unwrap_or_default();
    check(name.contains("Microsoft X-Box 360 pad") && name.contains("Vendor=045e Product=028e"), "it is an Xbox 360 pad to the kernel");

    let mut times = Vec::new();
    for i in 1..=200i16 {
        let value = (i % 2 * 2 - 1) * (1000 + i * 90);
        let t = Instant::now();
        unsafe { SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_LEFTX.0, value) };
        while abs(&pad, ABS_X) != i32::from(value) {
            assert!(t.elapsed() < Duration::from_millis(500), "the stick never arrived");
            std::hint::spin_loop();
        }
        times.push(t.elapsed());
        std::thread::sleep(Duration::from_micros(2000 + (i as u64 * 7919) % 5000));
    }
    times.sort();
    println!(
        "ok   stick moves reach the event node: median {:.2?}, p99 {:.2?}, max {:.2?} (n={})",
        times[times.len() / 2],
        times[times.len() * 99 / 100],
        times[times.len() - 1],
        times.len()
    );
    unsafe { SDL_SetJoystickVirtualAxis(joy, SDL_GAMEPAD_AXIS_LEFTY.0, -32768) };
    wait_for("stick up", Duration::from_millis(500), || (abs(&pad, ABS_Y) == -32768).then_some(()));
    check(true, "stick up stays up (negative Y, as xpad reports it)");
    unsafe { SDL_SetJoystickVirtualButton(joy, SDL_GAMEPAD_BUTTON_SOUTH.0, true) };
    wait_for("BTN_A", Duration::from_millis(500), || key(&pad, BTN_A).then_some(()));
    unsafe { SDL_SetJoystickVirtualButton(joy, SDL_GAMEPAD_BUTTON_SOUTH.0, false) };
    check(true, "the bottom face button is BTN_A");

    let paddle = |b: u8, down: bool| unsafe {
        SDL_SetJoystickVirtualButton(joy, BUTTON_MASK.count_ones() as i32 - (BUTTON_MASK >> b).count_ones() as i32, down)
    };
    paddle(RIGHT_PADDLE1, true);
    wait_for("the paddle as Y", Duration::from_millis(500), || key(&pad, BTN_Y).then_some(()));
    paddle(RIGHT_PADDLE1, false);
    wait_for("Y to be let go", Duration::from_millis(500), || (!key(&pad, BTN_Y)).then_some(()));
    check(true, "a paddle assigned to Y presses Y, and lets it go");

    let keyboard = wait_for("the keyboard", Duration::from_secs(5), || node(|_, name| name == "OpenController keyboard"));
    paddle(LEFT_PADDLE1, true);
    wait_for("F24 down", Duration::from_millis(500), || key(&keyboard, KEY_F24).then_some(()));
    paddle(LEFT_PADDLE1, false);
    wait_for("F24 up", Duration::from_millis(500), || (!key(&keyboard, KEY_F24)).then_some(()));
    check(true, "a paddle assigned to a key holds it on OpenController's keyboard while held");

    // A game's rumble: upload a force-feedback effect to the virtual pad and play it.
    let mut effect: libc::ff_effect = unsafe { std::mem::zeroed() };
    effect.type_ = 0x50; // FF_RUMBLE
    effect.id = -1;
    effect.u[0] = u64::from_ne_bytes([0x00, 0xC0, 0x00, 0x40, 0, 0, 0, 0]);
    let uploaded = unsafe { libc::ioctl(pad.as_raw_fd(), ioc(1, 0x80, size_of::<libc::ff_effect>()), &mut effect) };
    check(uploaded >= 0, "a game can upload a rumble effect");
    let play = libc::input_event { time: libc::timeval { tv_sec: 0, tv_usec: 0 }, type_: 0x15, code: effect.id as u16, value: 1 };
    let bytes = unsafe { std::slice::from_raw_parts((&play as *const libc::input_event).cast::<u8>(), size_of::<libc::input_event>()) };
    (&pad).write_all(bytes).expect("play");
    let rumble = wait_for("the rumble", Duration::from_secs(1), || Some(RUMBLE.load(Ordering::Acquire)).filter(|&r| r != 0));
    check(rumble >> 16 > 0 && rumble & 0xFFFF > 0, &format!("a game's rumble reaches the controller (low {:#06x}, high {:#06x})", rumble >> 16, rumble & 0xFFFF));

    engine.stop();
    std::thread::sleep(Duration::from_millis(300));
    check(node(|phys, _| phys.starts_with("open-controller/")).is_none(), "stopping removes the virtual controller");
}
