//! `--demo`: the window filled with example controllers, for screenshots and for working on
//! the interface without hardware. Only the view is faked; the engine runs as usual.

use open_controller_core::binding::{Action, Chord, Step, XboxButton, modifier};
use open_controller_core::device::{Brand, Link, Power};
use open_controller_core::extras::{self, Family, Features, Hint};
use open_controller_core::mapping::{PadState, axis, button};
use open_controller_core::profile::{Edit, Light, Profiles};
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

pub fn snapshot(real: &Snapshot, edits: &HashMap<String, Profiles>) -> Snapshot {
    let mut eightbitdo = pad(PadKey::Slot(1), "8BitDo Ultimate 2 Wireless", Brand::EightBitDo, Family::EightBitDoFour, (0x2DC8, 0x6012));
    eightbitdo.links = vec![Link::Dongle];
    eightbitdo.power = Power::Battery(Some(80));
    eightbitdo.role = Role::Virtual { player: Some(0) };
    eightbitdo.input = input(&[button::SOUTH, extras::LEFT_PADDLE1 as u32], [-9000, -14000, 2000, 0, 0, 30000]);
    eightbitdo.extras = vec![extras::LEFT_PADDLE1, extras::RIGHT_PADDLE1, extras::LEFT_PADDLE2, extras::RIGHT_PADDLE2];
    eightbitdo.features = Features { motion: true, rumble: true, ..Features::default() };
    eightbitdo.profiles = eightbitdo_profiles();

    let mut edge = pad(PadKey::Slot(2), "DualSense Edge Wireless Controller", Brand::PlayStation, Family::DualSenseEdge, (0x054C, 0x0DF2));
    edge.links = vec![Link::Bluetooth];
    edge.power = Power::Charging(Some(45));
    edge.role = Role::Virtual { player: Some(1) };
    edge.input = input(&[button::DPAD_LEFT, button::RIGHT_SHOULDER], [0, 0, 16000, 12000, 12000, 0]);
    edge.extras =
        vec![extras::LEFT_PADDLE1, extras::RIGHT_PADDLE1, extras::LEFT_PADDLE2, extras::RIGHT_PADDLE2, extras::MISC1, extras::TOUCHPAD];
    edge.features = Features { touchpad: true, motion: true, rumble: true, light_bar: true, player_lights: true, ..Features::default() };
    edge.can_power_off = true;
    edge.profiles.apply(Edit::Light(Light::Color([120, 0, 255])));

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

    let mut ds3 = pad(PadKey::Slot(6), "PS3 Controller", Brand::PlayStation, Family::DualShock3, (0x054C, 0x0268));
    ds3.role = Role::Virtual { player: None };
    ds3.features = Features { motion: true, rumble: true, player_lights: true, ..Features::default() };

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
    ds4.extras = vec![extras::TOUCHPAD];
    ds4.features = Features { touchpad: true, motion: true, rumble: true, light_bar: true, ..Features::default() };

    let mut xinput =
        pad(PadKey::Device(8), "8BitDo Ultimate 2 Wireless Controller for PC", Brand::EightBitDo, Family::EightBitDoFour, (0x2DC8, 0x310B));
    xinput.role = Role::Native { player: None };
    xinput.store = None;
    xinput.hidden = false;
    xinput.hint = Some(Hint::EightBitDoDInput);
    xinput.features = Features { rumble: true, ..Features::default() };

    let mut pads = vec![eightbitdo, edge, switch, joycons, ds3, xbox, ds4, xinput];
    for p in &mut pads {
        if let Some(edited) = p.store.as_ref().and_then(|k| edits.get(k)) {
            p.profiles = edited.clone();
        }
    }
    Snapshot {
        pads,
        vigem: Driver::Ready { version: Some("1.22.0".into()) },
        hidhide: Driver::Ready { version: None },
        hiding: true,
        sdl_version: real.sdl_version.clone(),
        sdl_error: None,
        running: real.running,
    }
}
