//! Windows: keys go out through `SendInput`. Each key carries its scan code as well as its
//! virtual-key code: games that read Raw Input or DirectInput look at the scan code.

use crate::binding::{Chord, modifier};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyNameTextW, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC_EX,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT, MapVirtualKeyW, SendInput, VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT, VkKeyScanW,
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
    v.extend(c.keys().map(|vk| key(vk, false)));
    v
}

fn up(c: Chord) -> Vec<INPUT> {
    let mut v = Vec::new();
    v.extend(c.keys().rev().map(|vk| key(vk, true)));
    v.extend(MODIFIERS.iter().rev().filter(|(m, _)| c.mods & m != 0).map(|&(_, vk)| key(vk, true)));
    v
}

fn key(vk: u16, release: bool) -> INPUT {
    if let Some((down, up, data)) = match vk {
        1 => Some((MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, 0)),
        2 => Some((MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, 0)),
        4 => Some((MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, 0)),
        5 => Some((MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, 1)),
        6 => Some((MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, 2)),
        _ => None,
    } {
        return INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT { dx: 0, dy: 0, mouseData: data, dwFlags: if release { up } else { down }, time: 0, dwExtraInfo: 0 },
            },
        };
    }
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
    fn multi_key_output_presses_every_key_and_releases_in_reverse_order() {
        let c = Chord::from_keys(modifier::CTRL, [0x56, 0x26]);
        let vks = |v: Vec<INPUT>| v.iter().map(|i| unsafe { (i.Anonymous.ki.wVk, i.Anonymous.ki.dwFlags) }).collect::<Vec<_>>();
        let pressed = vks(down(c));
        assert_eq!(pressed.iter().map(|i| i.0).collect::<Vec<_>>(), [VK_CONTROL, 0x56, 0x26]);
        assert!(pressed[2].1 & KEYEVENTF_EXTENDEDKEY != 0, "Up must arrive as the arrow, not the numpad key");
        assert!(pressed.iter().all(|i| i.1 & KEYEVENTF_KEYUP == 0));
        let released = vks(up(c));
        assert_eq!(released.iter().map(|i| i.0).collect::<Vec<_>>(), [0x26, 0x56, VK_CONTROL]);
        assert!(released.iter().all(|i| i.1 & KEYEVENTF_KEYUP != 0));
        assert_eq!(super::super::chord_names(c), ["Ctrl".to_string(), "V".to_string(), key_name(0x26)]);
    }

    #[test]
    fn chords_press_modifiers_first_and_release_them_last() {
        let c = Chord::new(modifier::CTRL | modifier::SHIFT, 0x41);
        let vks = |v: Vec<INPUT>| {
            v.iter().map(|i| unsafe { (i.Anonymous.ki.wVk, i.Anonymous.ki.dwFlags & KEYEVENTF_KEYUP != 0) }).collect::<Vec<_>>()
        };
        assert_eq!(vks(down(c)), [(VK_CONTROL, false), (VK_SHIFT, false), (0x41, false)]);
        assert_eq!(vks(up(c)), [(0x41, true), (VK_SHIFT, true), (VK_CONTROL, true)]);
    }

    #[test]
    fn obs_mouse4_chord_sends_modifiers_before_the_side_button_and_releases_them_last() {
        let c = Chord::new(modifier::CTRL | modifier::SHIFT, 5);
        let down = down(c);
        let up = up(c);
        assert_eq!(down.iter().map(|i| i.r#type).collect::<Vec<_>>(), [INPUT_KEYBOARD, INPUT_KEYBOARD, INPUT_MOUSE]);
        assert_eq!(unsafe { down[2].Anonymous.mi.dwFlags }, MOUSEEVENTF_XDOWN);
        assert_eq!(unsafe { down[2].Anonymous.mi.mouseData }, 1);
        assert_eq!(up.iter().map(|i| i.r#type).collect::<Vec<_>>(), [INPUT_MOUSE, INPUT_KEYBOARD, INPUT_KEYBOARD]);
        assert_eq!(unsafe { up[0].Anonymous.mi.dwFlags }, MOUSEEVENTF_XUP);
        assert_eq!(unsafe { up[0].Anonymous.mi.mouseData }, 1);
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
        assert_eq!(super::super::chord_names(Chord::new(modifier::CTRL, 0x70)), ["Ctrl", "F1"]);
        assert_eq!(vk_for_char('a'), Some(0x41));
    }
}
