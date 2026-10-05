//! `--demo`: the window filled with example controllers, for screenshots and for working on
//! the interface without hardware. Only the view is faked; the engine runs as usual.

use open_controller_core::device::{Brand, Link, Power};
use open_controller_core::mapping::{PadState, axis, button};
use open_controller_core::{Driver, PadKey, PadView, Role, Snapshot};
use std::time::Duration;

fn input(buttons: &[u32], axes: [i16; axis::COUNT]) -> PadState {
    PadState { buttons: buttons.iter().fold(0, |b, &i| b | 1 << i), axes }
}

pub fn snapshot(real: &Snapshot) -> Snapshot {
    let pad = |key, name: &str, brand, links: Vec<Link>, power, role, hidden, input, can_power_off| PadView {
        key,
        name: name.into(),
        brand,
        links,
        power,
        role,
        hidden,
        input,
        can_power_off,
    };
    Snapshot {
        pads: vec![
            pad(
                PadKey::Slot(1),
                "DualSense Wireless Controller",
                Brand::PlayStation,
                vec![Link::Bluetooth],
                Power::Battery(Some(80)),
                Role::Virtual { player: Some(0) },
                true,
                input(&[button::SOUTH], [-9000, -14000, 2000, 0, 0, 30000]),
                true,
            ),
            pad(
                PadKey::Slot(2),
                "Nintendo Switch Pro Controller",
                Brand::Nintendo,
                vec![Link::Usb, Link::Bluetooth],
                Power::Charging(Some(45)),
                Role::Virtual { player: Some(1) },
                true,
                input(&[button::DPAD_LEFT, button::RIGHT_SHOULDER], [0, 0, 16000, 12000, 12000, 0]),
                false,
            ),
            pad(
                PadKey::Device(7),
                "Xbox Series X Controller",
                Brand::Xbox,
                vec![Link::Wireless],
                Power::Battery(Some(60)),
                Role::Native { player: Some(2) },
                false,
                PadState::default(),
                false,
            ),
            pad(
                PadKey::Slot(3),
                "PS4 Controller",
                Brand::PlayStation,
                vec![Link::Dongle],
                Power::Unknown,
                Role::Waiting { player: Some(3), remaining: Duration::from_secs(9) },
                true,
                PadState::default(),
                false,
            ),
        ],
        vigem: Driver::Ready { version: Some("1.21.442.0".into()) },
        hidhide: Driver::Ready { version: None },
        hiding: true,
        sdl_version: real.sdl_version.clone(),
        sdl_error: None,
        running: real.running,
    }
}
