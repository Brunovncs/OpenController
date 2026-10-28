//! `--demo`: the window filled with example controllers, for screenshots and for working on
//! the interface without hardware. Only the view is faked; the engine runs as usual.

use open_controller_core::binding::{Action, Buttons, Chord, Step, XboxButton, modifier};
use open_controller_core::device::{Brand, Form, Link, Power};
use open_controller_core::extras::{self, Family, Features};
use open_controller_core::mapping::{PadState, RawInput, axis, button};
use open_controller_core::models;
use open_controller_core::profile::{DpadStick, Edit, Gyro, GyroMode, Light, Output, Pressure, Profiles};
use open_controller_core::{Driver, PadKey, PadView, Role, Snapshot};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

fn input(buttons: &[u32], axes: [i16; axis::COUNT]) -> PadState {
    PadState { buttons: buttons.iter().fold(0, |b, &i| b | 1 << i), axes, ..PadState::default() }
}

pub(crate) fn pad(key: PadKey, name: &str, brand: Brand, family: Family, (vendor, product): (u16, u16)) -> PadView {
    PadView {
        key,
        name: name.into(),
        brand,
        links: vec![Link::Usb],
        power: Power::Unknown,
        role: Role::Unmapped,
        hidden: true,
        input: PadState::default(),
        gyro: None,
        aim: (0, 0),
        can_power_off: false,
        vendor,
        product,
        family,
        art: extras::art(family, vendor, product),
        form: Form::Gamepad,
        raw: None,
        extras: Vec::new(),
        features: Features::default(),
        hint: None,
        store: Some(format!("demo:{vendor:04x}:{product:04x}")),
        profiles: Profiles::default(),
        in_use: 0,
        guid: format!("03000000{:02x}{:02x}0000{:02x}{:02x}000000000000", vendor & 0xff, vendor >> 8, product & 0xff, product >> 8),
        own_mapping: false,
    }
}

/// Profiles the example 8BitDo starts with, before anything is changed in the window.
fn eightbitdo_profiles() -> Profiles {
    let mut p = Profiles::default();
    p.apply(Edit::Bind { button: extras::LEFT_PADDLE2, action: Some(Action::Xbox(XboxButton::A)) });
    p.apply(Edit::Bind {
        button: extras::RIGHT_PADDLE2,
        action: Some(Action::Keys(Chord { mods: modifier::CTRL | modifier::SHIFT, key: 0x4D })),
    });
    p.apply(Edit::Bind {
        button: extras::RIGHT_PADDLE1,
        action: Some(Action::Macro(vec![
            Step::Keys(Chord { mods: 0, key: 0x31 }),
            Step::Wait(80),
            Step::Keys(Chord { mods: 0, key: 0x52 }),
        ])),
    });
    p.apply(Edit::Add("Racing".into()));
    p.apply(Edit::Select(0));
    p
}

pub fn snapshot(real: &Snapshot, edits: &HashMap<String, Profiles>, swaps: &[(PadKey, PadKey)], mapped: &[String]) -> Snapshot {
    let mut eightbitdo = pad(PadKey::Slot(1), "8BitDo Ultimate 2 Wireless", Brand::EightBitDo, Family::EightBitDoFour, (0x2DC8, 0x6012));
    eightbitdo.links = vec![Link::Dongle];
    eightbitdo.power = Power::Battery(Some(80));
    eightbitdo.role = Role::Virtual { player: Some(0) };
    eightbitdo.input = input(&[button::SOUTH, extras::LEFT_PADDLE1 as u32], [-9000, -14000, 2000, 0, 0, 30000]);
    eightbitdo.extras = vec![extras::LEFT_PADDLE1, extras::RIGHT_PADDLE1, extras::LEFT_PADDLE2, extras::RIGHT_PADDLE2];
    eightbitdo.features = Features { motion: true, rumble: true, ..Features::default() };
    eightbitdo.profiles = eightbitdo_profiles();

    let mut edge = pad(PadKey::Slot(2), "DualSense Wireless Controller", Brand::PlayStation, Family::DualSense, (0x054C, 0x0CE6));
    edge.links = vec![Link::Bluetooth];
    edge.power = Power::Charging(Some(45));
    edge.role = Role::Virtual { player: Some(1) };
    edge.input = input(&[button::DPAD_LEFT, button::RIGHT_SHOULDER], [0, 0, 16000, 12000, 12000, 0]);
    edge.extras = vec![
        extras::LEFT_PADDLE1,
        extras::RIGHT_PADDLE1,
        extras::LEFT_PADDLE2,
        extras::RIGHT_PADDLE2,
        extras::MISC1,
        extras::TOUCHPAD,
        extras::TOUCH_LEFT,
        extras::TOUCH_RIGHT,
        extras::TOUCH_TWO,
    ];
    edge.features = Features { touchpad: true, motion: true, rumble: true, light_bar: true, player_lights: true, ..Features::default() };
    edge.can_power_off = true;
    edge.profiles.apply(Edit::Light(Light::Color([120, 0, 255])));
    edge.profiles.apply(Edit::Gyro(Gyro { mode: GyroMode::Aiming, sensitivity: 150, ..Gyro::default() }));
    edge.profiles
        .apply(Edit::Bind { button: extras::LEFT_PADDLE1, action: Some(Action::Combo(Buttons::of(&[XboxButton::A, XboxButton::B]))) });
    edge.profiles.apply(Edit::Bind {
        button: extras::RIGHT_PADDLE1,
        action: Some(Action::Macro(vec![
            Step::Xbox(Buttons::of(&[XboxButton::Down])),
            Step::Xbox(Buttons::of(&[XboxButton::Right])),
            Step::Wait(40),
            Step::Xbox(Buttons::of(&[XboxButton::X, XboxButton::RightBumper])),
            Step::Keys(Chord { mods: 0, key: 0x46 }),
        ])),
    });

    let mut switch = pad(PadKey::Slot(3), "Nintendo Switch Pro Controller", Brand::Nintendo, Family::SwitchPro, (0x057E, 0x2009));
    switch.links = vec![Link::Usb, Link::Bluetooth];
    switch.power = Power::Charging(Some(90));
    switch.role = Role::Virtual { player: Some(2) };
    switch.extras = vec![extras::MISC1];
    switch.features = Features { motion: true, rumble: true, player_lights: true, ..Features::default() };
    switch.profiles.apply(Edit::Letters(true));
    switch.profiles.apply(Edit::Bind { button: button::BACK as u8, action: Some(Action::Keys(Chord { mods: 0, key: 0x4D })) });

    let mut joycons = pad(PadKey::Slot(5), "Joy-Con (L/R)", Brand::Nintendo, Family::JoyCons, (0x057E, 0x2008));
    joycons.links = vec![Link::Bluetooth];
    joycons.power = Power::Battery(Some(60));
    // XInput has four slots, all taken: the Joy-Con are a fifth controller.
    joycons.role = Role::Virtual { player: None };
    joycons.extras = vec![extras::LEFT_PADDLE1, extras::RIGHT_PADDLE1, extras::LEFT_PADDLE2, extras::RIGHT_PADDLE2, extras::MISC1];
    joycons.features = Features { motion: true, rumble: true, player_lights: true, ..Features::default() };

    let mut xbox = pad(PadKey::Device(7), "Xbox Series X Controller", Brand::Xbox, Family::Xbox, (0x045E, 0x0B13));
    xbox.links = vec![Link::Wireless];
    xbox.power = Power::Battery(Some(12));
    xbox.role = Role::Native { player: Some(3) };
    xbox.store = None;
    xbox.hidden = false;
    xbox.features = Features { rumble: true, ..Features::default() };

    // Kept native for a game that knows it, its settings shared by every Edge (no serial over USB).
    let mut native =
        pad(PadKey::Slot(6), "DualSense Edge Wireless Controller", Brand::PlayStation, Family::DualSenseEdge, (0x054C, 0x0DF2));
    native.power = Power::Charging(Some(70));
    native.role = Role::KeptNative;
    native.extras = vec![extras::LEFT_PADDLE1, extras::RIGHT_PADDLE1, extras::MISC1, extras::TOUCHPAD];
    native.features = Features {
        touchpad: true,
        motion: true,
        rumble: true,
        trigger_rumble: true,
        light_bar: true,
        player_lights: true,
        ..Features::default()
    };
    native.store = Some("model:054c:0df2".into());
    native.profiles.apply(Edit::Native(true));

    let mut ds4 = pad(PadKey::Slot(4), "PS4 Controller", Brand::PlayStation, Family::DualShock4, (0x054C, 0x09CC));
    ds4.links = vec![Link::Dongle];
    ds4.role = Role::Waiting { player: None, remaining: Duration::from_secs(9) };
    ds4.extras = vec![extras::TOUCHPAD, extras::TOUCH_LEFT, extras::TOUCH_RIGHT, extras::TOUCH_TWO];
    ds4.features = Features { touchpad: true, motion: true, rumble: true, light_bar: true, ..Features::default() };

    // Behind DsHidMini, shown to games as a DualShock 4 for its motion, Cross half pressed and
    // moving the right trigger.
    let mut ds3 = pad(PadKey::Slot(8), "DualShock 3", Brand::PlayStation, Family::DualShock3, (0x054C, 0x0268));
    ds3.role = Role::Virtual { player: None };
    ds3.power = Power::Charging(Some(80));
    ds3.input = input(&[button::SOUTH], [0, 0, -6000, 0, 0, 0]);
    ds3.input.pressure[0] = 128;
    ds3.gyro = Some([0.12, -0.4, 0.]);
    ds3.features = Features { motion: true, rumble: true, pressure: true, sixaxis: true, ..Features::default() };
    ds3.profiles.apply(Edit::Output(Output::DualShock4));
    ds3.profiles.apply(Edit::Pressure(Pressure { right_trigger: Some(button::SOUTH as u8), dpad: DpadStick::Left, ..Pressure::default() }));

    let mut pads = vec![eightbitdo, edge, switch, native, joycons, xbox, ds4, ds3];
    // Flight sticks only when asked for, so the screenshots stay as they are.
    if std::env::var_os("OPEN_CONTROLLER_DEMO_STICKS").is_some() {
        pads.extend(flight_sticks());
        pads.push(twin_shock(mapped));
    }
    for p in &mut pads {
        if let Some(edited) = p.store.as_ref().and_then(|k| edits.get(k)) {
            p.profiles = edited.clone();
        }
        p.in_use = p.profiles.active;
        // As the engine does: no virtual controller while it is kept native, one again after.
        match (p.profiles.keeps_native(p.form), &p.role) {
            (true, _) if matches!(p.key, PadKey::Slot(_)) => (p.role, p.hidden) = (Role::KeptNative, false),
            (false, Role::KeptNative) => (p.role, p.hidden) = (Role::Virtual { player: None }, true),
            _ => {}
        }
        // And by the name of the model chosen for it.
        if let Some(a) = models::alias(p.vendor, p.product, p.profiles.model.as_deref()) {
            p.name = a.4.into();
        }
    }
    // Players traded in the window, as the engine would.
    for &(a, b) in swaps {
        let (ia, ib) = (pads.iter().position(|p| p.key == a), pads.iter().position(|p| p.key == b));
        if let (Some(ia), Some(ib)) = (ia, ib) {
            let role = pads[ia].role.clone();
            pads[ia].role = pads[ib].role.clone();
            pads[ib].role = role;
        }
    }
    pads.sort_by_key(|p| match p.role {
        Role::Virtual { player: Some(n) } | Role::Waiting { player: Some(n), .. } | Role::Native { player: Some(n) } => u32::from(n),
        _ => 100,
    });
    Snapshot {
        pads,
        vigem: Driver::Ready { version: Some("1.22.0".into()) },
        hidhide: Driver::Ready { version: None },
        hiding: true,
        sdl_version: real.sdl_version.clone(),
        sdl_error: None,
        running: real.running,
        foreground: String::new(),
        ..Snapshot::default()
    }
}

/// A Logitech Extreme 3D Pro, left to flight games as it starts, leant forward and to the left,
/// twisted and with the throttle halfway; and a flight stick SDL has no layout for, its trigger
/// held.
fn flight_sticks() -> [PadView; 2] {
    let mut e3d = pad(PadKey::Slot(9), "Logitech Extreme 3D Pro", Brand::Other, Family::Other, (0x046D, 0xC215));
    e3d.form = Form::FlightStick;
    e3d.role = Role::KeptNative;
    e3d.hidden = false;
    e3d.store = Some("model:046d:c215".into());
    e3d.extras = vec![extras::MISC1];
    e3d.input = input(&[button::SOUTH, button::DPAD_UP], [-14000, -20000, 18000, 0, 0, 16000]);

    let mut t16000 = pad(PadKey::Device(10), "T.16000M", Brand::Other, Family::Other, (0x044F, 0xB10A));
    t16000.form = Form::FlightStick;
    t16000.hidden = false;
    t16000.store = None;
    t16000.raw = Some(RawInput { buttons: 1, axes: vec![1200, -8000, 300, 32767], hats: vec![0] });
    [e3d, t16000]
}

/// A PC twin shock pad SDL has no layout for, pressing every part in turn on its own so mapping
/// it can be tried; a gamepad once mapped in the window.
fn twin_shock(mapped: &[String]) -> PadView {
    let mut p = pad(PadKey::Device(11), "Microntek USB Joystick", Brand::Other, Family::Other, (0x0079, 0x0006));
    p.guid = "0300be79790000000600000000000000".into();
    if mapped.contains(&p.guid) {
        p.key = PadKey::Slot(11);
        p.role = Role::Virtual { player: None };
        p.store = Some("model:0079:0006".into());
        p.own_mapping = true;
    } else {
        p.hidden = false;
        p.store = None;
        p.raw = Some(pressing());
    }
    p
}

/// Its parts as a 0079:0006 pad numbers them, in the order a mapping asks for them, each held
/// for half a second with half a second between them, then a pause, over and over.
fn pressing() -> RawInput {
    static START: OnceLock<Instant> = OnceLock::new();
    let half_seconds = START.get_or_init(Instant::now).elapsed().as_millis() as usize / 500;
    let mut raw = RawInput { buttons: 0, axes: vec![0; 4], hats: vec![0] };
    let part = half_seconds / 2 % 24;
    if half_seconds % 2 == 1 {
        return raw;
    }
    match part {
        0..=3 => raw.buttons = 1 << [2, 1, 3, 0][part],
        4..=7 => raw.hats[0] = 1 << (part - 4),
        8..=11 => raw.buttons = 1 << (part - 4),
        12..=15 => raw.axes[part - 12] = i16::MAX,
        16..=19 => raw.buttons = 1 << [10, 11, 8, 9][part - 16],
        _ => {}
    }
    raw
}

/// What SDL might say about an example controller, for the report dialog in `--demo`.
pub fn diagnosis(p: &PadView) -> open_controller_core::report::Diagnosis {
    use open_controller_core::report::{Diagnosis, Facts, GyroFacts};
    let mode = p.profile().gyro.mode;
    if p.role == Role::Unmapped {
        return Diagnosis {
            facts: Facts {
                vendor: p.vendor,
                product: p.product,
                joystick_name: p.name.clone(),
                guid: "030000004f0400000ab1000000000000".into(),
                path: format!(r"\\?\HID#VID_{:04X}&PID_{:04X}", p.vendor, p.product),
                joystick_type: "flight stick".into(),
                connection: "wired".into(),
                axes: 4,
                buttons: 16,
                hats: 1,
                ..Facts::default()
            },
            log: vec![format!("     0.4s  joystick without a gamepad mapping {:04x}:{:04x} \"{}\" (example)", p.vendor, p.product, p.name)],
            gyro: None,
        };
    }
    Diagnosis {
        facts: Facts {
            vendor: p.vendor,
            product: p.product,
            joystick_name: p.name.clone(),
            gamepad_name: Some(p.name.clone()),
            guid: format!(
                "03000000{:02x}{:02x}0000{:02x}{:02x}000000000000",
                p.vendor & 0xff,
                p.vendor >> 8,
                p.product & 0xff,
                p.product >> 8
            ),
            path: format!(r"\\?\HID#VID_{:04X}&PID_{:04X}", p.vendor, p.product),
            gamepad_type: "standard".into(),
            real_type: "standard".into(),
            joystick_type: "gamepad".into(),
            connection: "wired".into(),
            axes: 6,
            buttons: 15,
            hats: 1,
            ..Facts::default()
        },
        log: vec![format!("     0.4s  added {:04x}:{:04x} \"{}\" (example)", p.vendor, p.product, p.name)],
        gyro: p.features.motion.then(|| GyroFacts {
            mode: format!("{mode:?}"),
            wanted: mode != GyroMode::Off,
            enabled: mode != GyroMode::Off,
            error: None,
            sixaxis: p.features.sixaxis.then(|| {
                "order Some(Driver), words Some([[ff, 01], [00, 02], [8f, 01], [fe, 01]]), yaw rest 0.00 °/s, failed reads 0, tilt true"
                    .into()
            }),
            output: Some(format!("{:?}", p.profiles.output)),
        }),
    }
}
