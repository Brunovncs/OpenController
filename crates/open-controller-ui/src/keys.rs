//! From what the window hears when a key is pressed to the virtual-key chord the engine sends.

use gpui::{Keystroke, Modifiers};
use open_controller_core::binding::{Chord, modifier};
use open_controller_core::keyboard;

pub fn mods(m: &Modifiers) -> u8 {
    let mut out = 0;
    if m.control {
        out |= modifier::CTRL;
    }
    if m.shift {
        out |= modifier::SHIFT;
    }
    if m.alt {
        out |= modifier::ALT;
    }
    if m.platform {
        out |= modifier::WIN;
    }
    out
}

/// GPUI's names for keys that type no character (`gpui_windows::events::parse_immutable`).
fn named(key: &str) -> Option<u16> {
    Some(match key {
        "space" => 0x20,
        "backspace" => 0x08,
        "enter" => 0x0D,
        "tab" => 0x09,
        "up" => 0x26,
        "down" => 0x28,
        "right" => 0x27,
        "left" => 0x25,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "escape" => 0x1B,
        "insert" => 0x2D,
        "delete" => 0x2E,
        "menu" => 0x5D,
        _ => {
            let n: u16 = key.strip_prefix('f')?.parse().ok()?;
            return (1..=24).contains(&n).then_some(0x6F + n);
        }
    })
}

pub fn chord(k: &Keystroke) -> Option<Chord> {
    let key = named(&k.key).or_else(|| {
        let mut chars = k.key.chars();
        let ch = chars.next()?;
        chars.next().is_none().then_some(())?;
        keyboard::vk_for_char(ch)
    })?;
    Some(Chord { mods: mods(&k.modifiers), key })
}

/// Keys the window cannot hear, because Windows acts on them first or the keyboard lacks them.
pub const OTHER: [u16; 21] = [
    0x5B, // Win
    0x2C, // Print Screen
    0xB3, 0xB0, 0xB1, 0xAD, 0xAE, 0xAF, // media and volume
    0x7C, 0x7D, 0x7E, 0x7F, 0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, // F13 to F24
    0x91, // Scroll Lock
];

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(key: &str, control: bool) -> Keystroke {
        Keystroke { modifiers: Modifiers { control, ..Default::default() }, key: key.into(), key_char: None }
    }

    #[test]
    fn keys() {
        assert_eq!(chord(&stroke("f5", false)), Some(Chord { mods: 0, key: 0x74 }));
        assert_eq!(chord(&stroke("f24", false)), Some(Chord { mods: 0, key: 0x87 }));
        assert_eq!(chord(&stroke("a", true)), Some(Chord { mods: modifier::CTRL, key: 0x41 }));
        assert_eq!(chord(&stroke("left", false)), Some(Chord { mods: 0, key: 0x25 }));
        assert_eq!(chord(&stroke("f25", false)), None);
    }
}
