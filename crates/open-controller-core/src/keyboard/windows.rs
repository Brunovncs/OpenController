//! Windows: keys go out through `SendInput`. Each key carries its scan code as well as its
//! virtual-key code: games that read Raw Input or DirectInput look at the scan code.

use crate::binding::{Chord, modifier};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyNameTextW, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC_EX,
    MapVirtualKeyW, SendInput, VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT, VkKeyScanW,
};

/// `SendInput` needs nothing opened first.
pub struct Sink;

impl Sink {
    pub fn open() -> Option<Sink> {
        Some(Sink)
    }

    pub fn down(&self, c: Chord) {
        send(&down(c));
    }

    pub fn up(&self, c: Chord) {
        send(&up(c));
    }
}

pub const META: &str = "Win";
pub const ALT: &str = "Alt";

const MODIFIERS: [(u8, u16); 4] =
    [(modifier::CTRL, VK_CONTROL), (modifier::SHIFT, VK_SHIFT), (modifier::ALT, VK_MENU), (modifier::WIN, VK_LWIN)];

fn down(c: Chord) -> Vec<INPUT> {
    let mut v: Vec<INPUT> = MODIFIERS.iter().filter(|(m, _)| c.mods & m != 0).map(|&(_, vk)| key(vk, false)).collect();
    if c.key != 0 {
        v.push(key(c.key, false));
    }
    v
}

fn up(c: Chord) -> Vec<INPUT> {
    let mut v = Vec::new();
    if c.key != 0 {
        v.push(key(c.key, true));
    }
    v.extend(MODIFIERS.iter().rev().filter(|(m, _)| c.mods & m != 0).map(|&(_, vk)| key(vk, true)));
    v
}

fn key(vk: u16, release: bool) -> INPUT {
    let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC_EX) };
    let mut flags = if release { KEYEVENTF_KEYUP } else { 0 };
    if is_extended(vk, scan) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: (scan & 0xFF) as u16, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

/// Keys that share a scan code with a number pad key and are told apart by the extended flag:
/// without it, the arrow keys arrive as the number pad's. Windows marks some of them with 0xE0
/// in the scan code it maps to, but not reliably, so they are listed too.
fn is_extended(vk: u16, scan: u32) -> bool {
    scan & 0xFF00 == 0xE000 || matches!(vk, 0x21..=0x28 | 0x2C..=0x2E | 0x5B..=0x5D | 0x6F | 0x90 | 0xA3 | 0xA5 | 0xA6..=0xB7)
}

fn send(inputs: &[INPUT]) {
    if !inputs.is_empty() {
        unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), size_of::<INPUT>() as i32) };
    }
}

/// The virtual-key code that types `ch` on the current keyboard layout, ignoring the modifiers
/// it needs. On a Brazilian ABNT2 keyboard `ç` is a key of its own, for one.
pub fn vk_for_char(ch: char) -> Option<u16> {
    let mut buf = [0u16; 2];
    let units = ch.encode_utf16(&mut buf);
    if units.len() != 1 {
        return None;
    }
    let r = unsafe { VkKeyScanW(units[0]) };
    (r != -1).then_some((r & 0xFF) as u16)
}

/// The key's name on the current layout, as Windows writes it ("Ctrl", "F5", "Ç").
pub fn key_name(vk: u16) -> String {
    if let Some(n) = super::fixed_name(vk) {
        return n.to_string();
    }
    let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC_EX) };
    let mut lparam = ((scan & 0xFF) as i32) << 16;
    if is_extended(vk, scan) {
        lparam |= 1 << 24;
    }
    let mut buf = [0u16; 64];
    let n = unsafe { GetKeyNameTextW(lparam, buf.as_mut_ptr(), buf.len() as i32) };
    if n > 0 {
        let s = String::from_utf16_lossy(&buf[..n as usize]);
        // Windows names letters in capitals and named keys in whatever case the layout has.
        let mut c = s.chars();
        return match c.next() {
            Some(f) if s.chars().count() > 1 => f.to_uppercase().chain(c.flat_map(|x| x.to_lowercase())).collect(),
            _ => s,
        };
    }
    format!("0x{vk:02X}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_press_modifiers_first_and_release_them_last() {
        let c = Chord { mods: modifier::CTRL | modifier::SHIFT, key: 0x41 };
        let vks = |v: Vec<INPUT>| {
            v.iter().map(|i| unsafe { (i.Anonymous.ki.wVk, i.Anonymous.ki.dwFlags & KEYEVENTF_KEYUP != 0) }).collect::<Vec<_>>()
        };
        assert_eq!(vks(down(c)), [(VK_CONTROL, false), (VK_SHIFT, false), (0x41, false)]);
        assert_eq!(vks(up(c)), [(0x41, true), (VK_SHIFT, true), (VK_CONTROL, true)]);
    }

    #[test]
    fn arrows_are_extended_keys() {
        let k = key(0x25, false);
        assert_ne!(unsafe { k.Anonymous.ki.dwFlags } & KEYEVENTF_EXTENDEDKEY, 0);
        let k = key(0x41, false);
        assert_eq!(unsafe { k.Anonymous.ki.dwFlags } & KEYEVENTF_EXTENDEDKEY, 0);
    }

    #[test]
    fn names() {
        assert_eq!(key_name(0x7C), "F13");
        assert_eq!(super::super::chord_names(Chord { mods: modifier::CTRL, key: 0x70 }), ["Ctrl", "F1"]);
        assert_eq!(vk_for_char('a'), Some(0x41));
    }
}
