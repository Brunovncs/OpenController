//! macOS: keys go out as Quartz events, which macOS allows once the user grants OpenController
//! the Accessibility permission. Key codes are positions, so a virtual-key code becomes the key in
//! its place on a US keyboard.

use crate::binding::{Chord, modifier};
use std::ffi::c_void;

pub const META: &str = "Cmd";
pub const ALT: &str = "Option";

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceCreate(state: i32) -> *mut c_void;
    fn CGEventCreateKeyboardEvent(source: *mut c_void, key: u16, down: bool) -> *mut c_void;
    fn CGEventSetFlags(event: *mut c_void, flags: u64);
    fn CGEventPost(tap: u32, event: *mut c_void);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: *const c_void);
}

const HID_SYSTEM_STATE: i32 = 1;
const HID_EVENT_TAP: u32 = 0;

/// (modifier, key code, event flag)
const MODIFIERS: [(u8, u16, u64); 4] =
    [(modifier::CTRL, 0x3B, 1 << 18), (modifier::SHIFT, 0x38, 1 << 17), (modifier::ALT, 0x3A, 1 << 19), (modifier::WIN, 0x37, 1 << 20)];

pub struct Sink {
    source: *mut c_void,
}

// The event source is only used from the keyboard thread that made it.
unsafe impl Send for Sink {}

impl Sink {
    pub fn open() -> Option<Sink> {
        let source = unsafe { CGEventSourceCreate(HID_SYSTEM_STATE) };
        (!source.is_null()).then_some(Sink { source })
    }

    fn post(&self, key: u16, down: bool, flags: u64) {
        unsafe {
            let ev = CGEventCreateKeyboardEvent(self.source, key, down);
            if ev.is_null() {
                return;
            }
            CGEventSetFlags(ev, flags);
            CGEventPost(HID_EVENT_TAP, ev);
            CFRelease(ev);
        }
    }

    pub fn down(&self, c: Chord) {
        let mut flags = 0;
        for &(m, key, flag) in &MODIFIERS {
            if c.mods & m != 0 {
                flags |= flag;
                self.post(key, true, flags);
            }
        }
        if let Some(k) = code(c.key) {
            self.post(k, true, flags);
        }
    }

    pub fn up(&self, c: Chord) {
        let mut flags: u64 = MODIFIERS.iter().filter(|(m, ..)| c.mods & m != 0).map(|&(.., f)| f).sum();
        if let Some(k) = code(c.key) {
            self.post(k, false, flags);
        }
        for &(m, key, flag) in MODIFIERS.iter().rev() {
            if c.mods & m != 0 {
                flags &= !flag;
                self.post(key, false, flags);
            }
        }
    }
}

impl Drop for Sink {
    fn drop(&mut self) {
        unsafe { CFRelease(self.source) };
    }
}

/// The macOS key code (`kVK_*`) in the place of a virtual-key code on a US keyboard.
fn code(vk: u16) -> Option<u16> {
    const LETTERS: [u16; 26] = [
        0x00, 0x0B, 0x08, 0x02, 0x0E, 0x03, 0x05, 0x04, 0x22, 0x26, 0x28, 0x25, 0x2E, 0x2D, 0x1F, 0x23, 0x0C, 0x0F, 0x01, 0x11, 0x20, 0x09,
        0x0D, 0x07, 0x10, 0x06,
    ];
    const DIGITS: [u16; 10] = [0x1D, 0x12, 0x13, 0x14, 0x15, 0x17, 0x16, 0x1A, 0x1C, 0x19];
    const NUMPAD: [u16; 10] = [0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5B, 0x5C];
    const F_KEYS: [u16; 20] =
        [0x7A, 0x78, 0x63, 0x76, 0x60, 0x61, 0x62, 0x64, 0x65, 0x6D, 0x67, 0x6F, 0x69, 0x6B, 0x71, 0x6A, 0x40, 0x4F, 0x50, 0x5A];
    Some(match vk {
        0x08 => 0x33,
        0x09 => 0x30,
        0x0D => 0x24,
        0x10 | 0xA0 => 0x38,
        0xA1 => 0x3C,
        0x11 | 0xA2 => 0x3B,
        0xA3 => 0x3E,
        0x12 | 0xA4 => 0x3A,
        0xA5 => 0x3D,
        0x14 => 0x39,
        0x1B => 0x35,
        0x20 => 0x31,
        0x21 => 0x74,
        0x22 => 0x79,
        0x23 => 0x77,
        0x24 => 0x73,
        0x25 => 0x7B,
        0x26 => 0x7E,
        0x27 => 0x7C,
        0x28 => 0x7D,
        0x2D => 0x72,
        0x2E => 0x75,
        0x30..=0x39 => DIGITS[(vk - 0x30) as usize],
        0x41..=0x5A => LETTERS[(vk - 0x41) as usize],
        0x5B | 0x5C => 0x37,
        0x60..=0x69 => NUMPAD[(vk - 0x60) as usize],
        0x6A => 0x43,
        0x6B => 0x45,
        0x6D => 0x4E,
        0x6E => 0x41,
        0x6F => 0x4B,
        0x70..=0x83 => F_KEYS[(vk - 0x70) as usize],
        0xAD => 0x4A,
        0xAE => 0x49,
        0xAF => 0x48,
        0xBA => 0x29,
        0xBB => 0x18,
        0xBC => 0x2B,
        0xBD => 0x1B,
        0xBE => 0x2F,
        0xBF => 0x2C,
        0xC0 => 0x32,
        0xDB => 0x21,
        0xDC => 0x2A,
        0xDD => 0x1E,
        0xDE => 0x27,
        0xE2 => 0x0A,
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
        assert_eq!(code(0x41), Some(0x00)); // A
        assert_eq!(code(0x5A), Some(0x06)); // Z
        assert_eq!(code(0x31), Some(0x12)); // 1
        assert_eq!(code(0x70), Some(0x7A)); // F1
        assert_eq!(code(0x83), Some(0x5A)); // F20
        assert_eq!(code(0x84), None); // F21
    }
}
