//! `--demo`: the window filled with example controllers, for screenshots and for working on
//! the interface without hardware. Only the view is faked; the engine runs as usual.

use open_controller_core::binding::{Action, Chord, Step, XboxButton, modifier};
use open_controller_core::device::{Brand, Link, Power};
use open_controller_core::extras::{self, Family, Features};
use open_controller_core::mapping::{PadState, axis, button};
use open_controller_core::profile::{Edit, Gyro, GyroMode, Light, Profiles};
use open_controller_core::{Driver, PadKey, PadView, Role, Snapshot};
use std::collections::HashMap;
use std::time::Duration;

fn input(buttons: &[u32], axes: [i16; axis::COUNT]) -> PadState {
    PadState { buttons: buttons.iter().fold(0, |b, &i| b | 1 << i), axes }
}

fn pad(key: PadKey, name: &str, brand: Brand, family: Family, (vendor, product): (u16, u16)) -> PadView {
    PadView {
        key,
        name: name.into(),
        brand,
        links: vec![Link::Usb],
        power: Power::Unknown,
        role: Role::Unmapped,
        hidden: true,
        input: PadState::default(),
        can_power_off: false,
        vendor,
        product,
        family,
        art: extras::art(family, vendor, product),
        extras: Vec::new(),
        features: Features::default(),
        hint: None,
        store: Some(format!("demo:{vendor:04x}:{product:04x}")),
        profiles: Profiles::default(),
        in_use: 0,
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

pub fn snapshot(real: &Snapshot, edits: &HashMap<String, Profiles>, swaps: &[(PadKey, PadKey)]) -> Snapshot {
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
    edge.profiles.apply(Edit::Gyro(Gyro { mode: GyroMode::Aiming, sensitivity: 150, invert_y: false }));

    let mut switch = pad(PadKey::Slot(3), "Nintendo Switch Pro Controller", Brand::Nintendo, Family::SwitchPro, (0x057E, 0x2009));
    switch.links = vec![Link::Usb, Link::Bluetooth];
    switch.power = Power::Charging(Some(90));
    switch.role = Role::Virtual { player: Some(2) };
    switch.extras = vec![extras::MISC1];
    switch.features = Features { motion: true, rumble: true, player_lights: true, ..Features::default() };

    let mut joycons = pad(PadKey::Slot(5), "Joy-Con (L/R)", Brand::Nintendo, Family::JoyCons, (0x057E, 0x2008));
    joycons.links = vec![Link::Bluetooth];
    joycons.power = Power::Battery(Some(60));
    joycons.role = Role::Virtual { player: Some(3) };
    joycons.extras = vec![extras::LEFT_PADDLE1, extras::RIGHT_PADDLE1, extras::LEFT_PADDLE2, extras::RIGHT_PADDLE2, extras::MISC1];
    joycons.features = Features { motion: true, rumble: true, player_lights: true, ..Features::default() };

    let mut sn30 = pad(PadKey::Slot(6), "8BitDo SN30 Pro", Brand::EightBitDo, Family::EightBitDo, (0x2DC8, 0x6001));
    sn30.links = vec![Link::Bluetooth];
    sn30.power = Power::Battery(Some(70));
    sn30.role = Role::Virtual { player: None };
    sn30.features = Features { motion: true, rumble: true, ..Features::default() };

    let mut xbox = pad(PadKey::Device(7), "Xbox Series X Controller", Brand::Xbox, Family::Xbox, (0x045E, 0x0B13));
    xbox.links = vec![Link::Wireless];
    xbox.power = Power::Battery(Some(12));
    xbox.role = Role::Native { player: Some(3) };
    xbox.store = None;
    xbox.hidden = false;
    xbox.features = Features { rumble: true, ..Features::default() };

    let mut ds4 = pad(PadKey::Slot(4), "PS4 Controller", Brand::PlayStation, Family::DualShock4, (0x054C, 0x09CC));
    ds4.links = vec![Link::Dongle];
    ds4.role = Role::Waiting { player: None, remaining: Duration::from_secs(9) };
    ds4.extras = vec![extras::TOUCHPAD, extras::TOUCH_LEFT, extras::TOUCH_RIGHT, extras::TOUCH_TWO];
    ds4.features = Features { touchpad: true, motion: true, rumble: true, light_bar: true, ..Features::default() };

    let mut pro = pad(PadKey::Slot(8), "DualSense Edge Wireless Controller", Brand::PlayStation, Family::DualSenseEdge, (0x054C, 0x0DF2));
    pro.links = vec![Link::Usb];
    pro.power = Power::Charged;
    pro.role = Role::Virtual { player: None };
    pro.extras = vec![extras::LEFT_PADDLE1, extras::RIGHT_PADDLE1, extras::TOUCHPAD, extras::TOUCH_LEFT, extras::TOUCH_RIGHT];
    pro.features = Features { touchpad: true, motion: true, rumble: true, light_bar: true, player_lights: true, ..Features::default() };

    let mut pads = vec![eightbitdo, edge, switch, joycons, sn30, xbox, ds4, pro];
    for p in &mut pads {
        if let Some(edited) = p.store.as_ref().and_then(|k| edits.get(k)) {
            p.profiles = edited.clone();
        }
        p.in_use = p.profiles.active;
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
        Role::Virtual { player: Some(n) } | Role::Waiting { player: Some(n), .. } => u32::from(n),
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
    }
}
