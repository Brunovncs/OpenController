//! Linux: keys go out through a uinput keyboard of Open Controller's own, which the desktop and
//! games take for one more keyboard. Key codes are positions, so a virtual-key code becomes the
//! key in its place on a US keyboard.

use crate::binding::{Chord, modifier};
use crate::linux::uinput::{EV_KEY, EV_SYN, UI_DEV_CREATE, UI_DEV_DESTROY, UI_DEV_SETUP, UI_SET_EVBIT, UI_SET_KEYBIT, event, write_events};
use std::ffi::c_int;
use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;

pub const META: &str = "Super";
pub const ALT: &str = "Alt";

const BUS_VIRTUAL: u16 = 0x06;

const MODIFIERS: [(u8, u16); 4] = [(modifier::CTRL, 29), (modifier::SHIFT, 42), (modifier::ALT, 56), (modifier::WIN, 125)];

pub struct Sink {
    file: File,
}

impl Sink {
    pub fn open() -> Option<Sink> {
        let file = OpenOptions::new().write(true).custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC).open("/dev/uinput").ok()?;
        let fd = file.as_raw_fd();
        unsafe {
            if libc::ioctl(fd, UI_SET_EVBIT, c_int::from(EV_KEY)) < 0 {
                return None;
            }
            for k in 1..=248 {
                libc::ioctl(fd, UI_SET_KEYBIT, k as c_int);
            }
            let mut setup: libc::uinput_setup = std::mem::zeroed();
            setup.id = libc::input_id { bustype: BUS_VIRTUAL, vendor: 0, product: 0, version: 1 };
            for (dst, &src) in setup.name.iter_mut().zip(b"Open Controller keyboard") {
                *dst = src as libc::c_char;
            }
            if libc::ioctl(fd, UI_DEV_SETUP, &setup) < 0 || libc::ioctl(fd, UI_DEV_CREATE) < 0 {
                return None;
            }
        }
        Some(Sink { file })
    }

    fn keys(&self, codes: impl Iterator<Item = u16>, value: i32) {
        // One report per key, so a modifier is down before the key it modifies arrives.
        let events: Vec<libc::input_event> = codes.flat_map(|c| [event(EV_KEY, c, value), event(EV_SYN, 0, 0)]).collect();
        let _ = write_events(&self.file, &events);
    }

    pub fn down(&self, c: Chord) {
        let mods = MODIFIERS.iter().filter(|(m, _)| c.mods & m != 0).map(|&(_, k)| k);
        self.keys(mods.chain(code(c.key)), 1);
    }

    pub fn up(&self, c: Chord) {
        let mods = MODIFIERS.iter().rev().filter(|(m, _)| c.mods & m != 0).map(|&(_, k)| k);
        self.keys(code(c.key).into_iter().chain(mods), 0);
    }
}

impl Drop for Sink {
    fn drop(&mut self) {
        unsafe { libc::ioctl(self.file.as_raw_fd(), UI_DEV_DESTROY) };
    }
}

/// The Linux key code (`KEY_*`) in the place of a virtual-key code on a US keyboard.
fn code(vk: u16) -> Option<u16> {
    const LETTERS: [u16; 26] = [30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17, 45, 21, 44];
    const NUMPAD: [u16; 10] = [82, 79, 80, 81, 75, 76, 77, 71, 72, 73];
    Some(match vk {
        0 => return None,
        0x08 => 14,
        0x09 => 15,
        0x0D => 28,
        0x10 | 0xA0 => 42,
        0xA1 => 54,
        0x11 | 0xA2 => 29,
        0xA3 => 97,
        0x12 | 0xA4 => 56,
        0xA5 => 100,
        0x13 => 119,
        0x14 => 58,
        0x1B => 1,
        0x20 => 57,
        0x21 => 104,
        0x22 => 109,
        0x23 => 107,
        0x24 => 102,
        0x25 => 105,
        0x26 => 103,
        0x27 => 106,
        0x28 => 108,
        0x2C => 99,
        0x2D => 110,
        0x2E => 111,
        0x30 => 11,
        0x31..=0x39 => vk - 0x31 + 2,
        0x41..=0x5A => LETTERS[(vk - 0x41) as usize],
        0x5B => 125,
        0x5C => 126,
        0x5D => 127,
        0x60..=0x69 => NUMPAD[(vk - 0x60) as usize],
        0x6A => 55,
        0x6B => 78,
        0x6D => 74,
        0x6E => 83,
        0x6F => 98,
        0x70..=0x79 => vk - 0x70 + 59,
        0x7A => 87,
        0x7B => 88,
        0x7C..=0x87 => vk - 0x7C + 183,
        0x90 => 69,
        0x91 => 70,
        0xAD => 113,
        0xAE => 114,
        0xAF => 115,
        0xB0 => 163,
        0xB1 => 165,
        0xB2 => 166,
        0xB3 => 164,
        0xBA => 39,
        0xBB => 13,
        0xBC => 51,
        0xBD => 12,
        0xBE => 52,
        0xBF => 53,
        0xC0 => 41,
        0xC1 => 89,
        0xC2 => 121,
        0xDB => 26,
        0xDC => 43,
        0xDD => 27,
        0xDE => 40,
        0xE2 => 86,
        _ => return None,
    })
}

pub fn key_name(vk: u16) -> String {
    super::plain_name(vk)
}

pub fn vk_for_char(ch: char) -> Option<u16> {
    super::us_vk_for_char(ch)
}

#[cfg(test)]
mod tests {
    use super::code;

    #[test]
    fn key_codes() {
        assert_eq!(code(0x41), Some(30)); // A
        assert_eq!(code(0x5A), Some(44)); // Z
        assert_eq!(code(0x30), Some(11)); // 0
        assert_eq!(code(0x31), Some(2)); // 1
        assert_eq!(code(0x70), Some(59)); // F1
        assert_eq!(code(0x7B), Some(88)); // F12
        assert_eq!(code(0x7C), Some(183)); // F13
        assert_eq!(code(0x87), Some(194)); // F24
        assert_eq!(code(0x25), Some(105)); // Left
        assert_eq!(code(0), None);
    }
}
