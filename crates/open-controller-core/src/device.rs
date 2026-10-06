//! What a connected controller is: its maker, how it is connected and what identifies it across
//! reconnections. Pure functions over what SDL reports, so they can be tested without hardware.

use serde::{Deserialize, Serialize};

/// How a controller reaches the computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Link {
    Usb,
    Bluetooth,
    /// A vendor receiver, such as Sony's DualShock 4 USB wireless adaptor.
    Dongle,
    /// Wireless, but the system does not say whether through Bluetooth or a receiver. XInput
    /// reports Xbox controllers this way.
    Wireless,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Brand {
    PlayStation,
    Xbox,
    Nintendo,
    EightBitDo,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Power {
    Unknown,
    /// Running on battery, with the charge in percent when the controller reports it.
    Battery(Option<u8>),
    Charging(Option<u8>),
    Charged,
    /// Powered by the cable, without a battery.
    Wired,
}

/// SDL's `SDL_JoystickConnectionState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SdlConnection {
    Unknown,
    Wired,
    Wireless,
}

/// Receivers that present the controller behind them as a USB device of their own.
const DONGLES: &[(u16, u16)] = &[
    (0x054C, 0x0BA0), // Sony DualShock 4 USB wireless adaptor
    (0x045E, 0x0719), // Microsoft Xbox 360 wireless receiver
];

/// Bluetooth HID devices carry the service class in their device path: HID over classic
/// Bluetooth (0x1124) or over Bluetooth LE (HOGP, 0x1812).
fn is_bluetooth_path(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    p.contains("00001124-0000-1000-8000-00805f9b34fb") || p.contains("00001812-0000-1000-8000-00805f9b34fb")
}

pub fn link(vendor: u16, product: u16, path: &str, sdl: SdlConnection) -> Link {
    if DONGLES.contains(&(vendor, product)) {
        return Link::Dongle;
    }
    if is_bluetooth_path(path) {
        return Link::Bluetooth;
    }
    match sdl {
        SdlConnection::Wired => Link::Usb,
        SdlConnection::Wireless => Link::Wireless,
        SdlConnection::Unknown => Link::Unknown,
    }
}

/// SDL's `SDL_GamepadType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadType {
    Standard,
    Xbox360,
    XboxOne,
    Ps3,
    Ps4,
    Ps5,
    SwitchPro,
    JoyConLeft,
    JoyConRight,
    JoyConPair,
    GameCube,
    Unknown,
}

pub fn brand(vendor: u16, ty: PadType) -> Brand {
    match vendor {
        0x2DC8 => return Brand::EightBitDo,
        0x054C => return Brand::PlayStation,
        0x045E => return Brand::Xbox,
        0x057E => return Brand::Nintendo,
        _ => {}
    }
    match ty {
        PadType::Ps3 | PadType::Ps4 | PadType::Ps5 => Brand::PlayStation,
        PadType::Xbox360 | PadType::XboxOne => Brand::Xbox,
        PadType::SwitchPro | PadType::JoyConLeft | PadType::JoyConRight | PadType::JoyConPair | PadType::GameCube => Brand::Nintendo,
        _ => Brand::Other,
    }
}

/// What identifies one physical controller, whichever way it is connected right now.
///
/// PlayStation, Switch and most Bluetooth-capable pads report their Bluetooth address as the
/// serial over every transport, so a DualSense keeps its identity when it goes from Bluetooth to
/// the cable. Without a serial the device path is the best there is: stable while the pad stays
/// on the same port or pairing, different otherwise.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Identity {
    Serial(String),
    Path(String),
}

pub fn identity(vendor: u16, product: u16, serial: Option<&str>, path: &str) -> Identity {
    if let Some(s) = serial.map(normalise_serial).filter(|s| is_meaningful(s)) {
        return Identity::Serial(s);
    }
    Identity::Path(format!("{vendor:04x}:{product:04x}:{}", path.to_ascii_lowercase()))
}

/// Lower-case hex digits and letters only, so `AA:BB:CC:DD:EE:FF`, `aa-bb-cc-dd-ee-ff` and
/// `aabbccddeeff` are one controller.
fn normalise_serial(s: &str) -> String {
    s.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect()
}

/// Clones and receivers often report an empty or all-zero serial, shared by every unit.
fn is_meaningful(s: &str) -> bool {
    s.len() >= 6 && s.chars().any(|c| c != '0' && c != 'f')
}

/// A serial that is a Bluetooth address, as a `BLUETOOTH_ADDRESS` value (`AA:BB:..` is
/// `0xAABB..`).
pub fn bluetooth_address(id: &Identity) -> Option<u64> {
    match id {
        Identity::Serial(s) if s.len() == 12 => u64::from_str_radix(s, 16).ok(),
        _ => None,
    }
}

/// XInput devices are already visible to every game, so they are shown but not doubled.
/// SDL reads them through its XInput backend, whose device paths are `XInput#<slot>`, or
/// through Raw Input, whose paths carry the `IG_` interface marker.
pub fn is_xinput_path(path: &str) -> bool {
    path.starts_with("XInput#") || path.to_ascii_uppercase().contains("&IG_")
}

/// The XInput slot in an `XInput#<slot>` path.
pub fn xinput_slot(path: &str) -> Option<u8> {
    path.strip_prefix("XInput#")?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BT_DS4: &str =
        r"\\?\HID#{00001124-0000-1000-8000-00805f9b34fb}_VID&0002054c_PID&09cc#9&1b0a8b5c&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}";
    const USB_DS4: &str = r"\\?\HID#VID_054C&PID_09CC&MI_03#7&2b5c3c1a&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}";

    #[test]
    fn links() {
        assert_eq!(link(0x054C, 0x09CC, BT_DS4, SdlConnection::Wireless), Link::Bluetooth);
        assert_eq!(link(0x054C, 0x09CC, USB_DS4, SdlConnection::Wired), Link::Usb);
        assert_eq!(link(0x054C, 0x0BA0, USB_DS4, SdlConnection::Wireless), Link::Dongle);
        assert_eq!(link(0x045E, 0x0B13, "XInput#0", SdlConnection::Wireless), Link::Wireless);
        assert_eq!(link(0x045E, 0x0B12, "XInput#1", SdlConnection::Wired), Link::Usb);
    }

    #[test]
    fn one_identity_across_transports() {
        let bt = identity(0x054C, 0x0CE6, Some("A0:5A:5C:12:34:56"), "bt-path");
        let usb = identity(0x054C, 0x0CE6, Some("a05a5c123456"), "usb-path");
        assert_eq!(bt, usb);
        assert_eq!(bluetooth_address(&bt), Some(0xA05A_5C12_3456));
    }

    #[test]
    fn placeholder_serials_fall_back_to_the_path() {
        for s in [None, Some(""), Some("000000000000"), Some("ff:ff:ff:ff:ff:ff"), Some("12")] {
            assert!(matches!(identity(1, 2, s, "P"), Identity::Path(_)), "{s:?}");
        }
        assert_eq!(identity(1, 2, None, "P"), Identity::Path("0001:0002:p".into()));
    }

    #[test]
    fn brands() {
        assert_eq!(brand(0x054C, PadType::Ps5), Brand::PlayStation);
        assert_eq!(brand(0x2DC8, PadType::XboxOne), Brand::EightBitDo);
        assert_eq!(brand(0x0F0D, PadType::SwitchPro), Brand::Nintendo);
        assert_eq!(brand(0x24C6, PadType::XboxOne), Brand::Xbox);
        assert_eq!(brand(0x0079, PadType::Standard), Brand::Other);
    }

    #[test]
    fn xinput_paths() {
        assert!(is_xinput_path("XInput#2"));
        assert!(is_xinput_path(r"\\?\HID#VID_045E&PID_028E&IG_00#3&1"));
        assert!(!is_xinput_path(USB_DS4));
        assert_eq!(xinput_slot("XInput#3"), Some(3));
        assert_eq!(xinput_slot(USB_DS4), None);
    }
}
