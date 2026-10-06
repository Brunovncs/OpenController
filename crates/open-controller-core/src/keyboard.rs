//! Keys pressed on the user's behalf for extra buttons, and key names for the interface.
//!
//! Keys go out from a thread of their own, so a macro's waits never hold up the input thread.
//! Chords are Windows virtual-key codes on every system, so settings carry over between them;
//! each system's module turns them into its own key codes.

use crate::binding::{Chord, Step, modifier};
use crossbeam_channel::{Sender, unbounded};
use std::time::Duration;

#[cfg_attr(windows, path = "keyboard/windows.rs")]
#[cfg_attr(target_os = "linux", path = "keyboard/linux.rs")]
#[cfg_attr(target_os = "macos", path = "keyboard/macos.rs")]
mod sys;

pub use sys::{key_name, vk_for_char};

enum Out {
    Down(Chord),
    Up(Chord),
    Play(Vec<Step>),
}

/// The thread that types. Dropping it lets the thread finish what is queued and end.
pub struct Keyboard {
    tx: Sender<Out>,
}

impl Keyboard {
    pub fn start() -> Keyboard {
        let (tx, rx) = unbounded::<Out>();
        std::thread::Builder::new()
            .name("open-controller-keys".into())
            .spawn(move || {
                // Without a way to type (Linux without the device rule), keys are dropped.
                let sink = sys::Sink::open();
                while let Ok(out) = rx.recv() {
                    let Some(sink) = sink.as_ref() else { continue };
                    match out {
                        Out::Down(c) => sink.down(c),
                        Out::Up(c) => sink.up(c),
                        Out::Play(steps) => play(sink, &steps),
                    }
                }
            })
            .expect("could not start the keyboard thread");
        Keyboard { tx }
    }

    pub fn press(&self, c: Chord) {
        let _ = self.tx.send(Out::Down(c));
    }

    pub fn release(&self, c: Chord) {
        let _ = self.tx.send(Out::Up(c));
    }

    pub fn play(&self, steps: Vec<Step>) {
        let _ = self.tx.send(Out::Play(steps));
    }
}

/// Gap between the keys of a macro step, so programs that poll the keyboard see each one.
const TAP: Duration = Duration::from_millis(15);

fn play(sink: &sys::Sink, steps: &[Step]) {
    for s in steps {
        match *s {
            Step::Keys(c) => {
                sink.down(c);
                std::thread::sleep(TAP);
                sink.up(c);
                std::thread::sleep(TAP);
            }
            Step::Wait(ms) => std::thread::sleep(Duration::from_millis(ms as u64)),
        }
    }
}

/// Keys whose system name is missing or unhelpful.
fn fixed_name(vk: u16) -> Option<&'static str> {
    Some(match vk {
        0x5B | 0x5C => sys::META,
        0xAD => "Mute",
        0xAE => "Volume −",
        0xAF => "Volume +",
        0xB0 => "Next track",
        0xB1 => "Previous track",
        0xB3 => "Play/Pause",
        0x2C => "Print Screen",
        0x7C..=0x87 => return F_KEYS.get((vk - 0x7C) as usize).copied(),
        _ => return None,
    })
}

const F_KEYS: [&str; 12] = ["F13", "F14", "F15", "F16", "F17", "F18", "F19", "F20", "F21", "F22", "F23", "F24"];

/// A key's name where the system gives none: what a US keyboard prints on it.
#[cfg_attr(windows, allow(dead_code))]
fn plain_name(vk: u16) -> String {
    if let Some(n) = fixed_name(vk) {
        return n.to_string();
    }
    let named = match vk {
        0x08 => "Backspace",
        0x09 => "Tab",
        0x0D => "Enter",
        0x10 | 0xA0 | 0xA1 => "Shift",
        0x11 | 0xA2 | 0xA3 => "Ctrl",
        0x12 | 0xA4 | 0xA5 => "Alt",
        0x13 => "Pause",
        0x14 => "Caps Lock",
        0x1B => "Esc",
        0x20 => "Space",
        0x21 => "Page Up",
        0x22 => "Page Down",
        0x23 => "End",
        0x24 => "Home",
        0x25 => "Left",
        0x26 => "Up",
        0x27 => "Right",
        0x28 => "Down",
        0x2D => "Insert",
        0x2E => "Delete",
        0x5D => "Menu",
        0x6A => "Num *",
        0x6B => "Num +",
        0x6D => "Num -",
        0x6E => "Num .",
        0x6F => "Num /",
        0x90 => "Num Lock",
        0x91 => "Scroll Lock",
        _ => "",
    };
    if !named.is_empty() {
        return named.into();
    }
    match vk {
        0x30..=0x39 | 0x41..=0x5A => char::from(vk as u8).to_string(),
        0x60..=0x69 => format!("Num {}", vk - 0x60),
        0x70..=0x7B => format!("F{}", vk - 0x6F),
        _ => US_PUNCTUATION.iter().find(|&&(_, v)| v == vk).map_or_else(|| format!("0x{vk:02X}"), |(c, _)| c.to_string()),
    }
}

/// The punctuation keys of a US keyboard and their virtual-key codes.
const US_PUNCTUATION: [(char, u16); 11] = [
    (';', 0xBA),
    ('=', 0xBB),
    (',', 0xBC),
    ('-', 0xBD),
    ('.', 0xBE),
    ('/', 0xBF),
    ('`', 0xC0),
    ('[', 0xDB),
    ('\\', 0xDC),
    (']', 0xDD),
    ('\'', 0xDE),
];

/// The virtual-key code of the US key that types `ch`, for systems without a layout lookup.
#[cfg_attr(windows, allow(dead_code))]
fn us_vk_for_char(ch: char) -> Option<u16> {
    let up = ch.to_ascii_uppercase();
    match up {
        'A'..='Z' | '0'..='9' => Some(up as u16),
        _ => US_PUNCTUATION.iter().find(|(c, _)| *c == ch).map(|&(_, v)| v),
    }
}

/// The chord as keycaps, modifiers first.
pub fn chord_names(c: Chord) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for (m, name) in [(modifier::CTRL, "Ctrl"), (modifier::SHIFT, "Shift"), (modifier::ALT, sys::ALT), (modifier::WIN, sys::META)] {
        if c.mods & m != 0 {
            v.push(name.into());
        }
    }
    if c.key != 0 {
        v.push(key_name(c.key));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_names() {
        assert_eq!(plain_name(0x41), "A");
        assert_eq!(plain_name(0x74), "F5");
        assert_eq!(plain_name(0x7C), "F13");
        assert_eq!(plain_name(0x25), "Left");
        assert_eq!(plain_name(0xBF), "/");
        assert_eq!(us_vk_for_char('a'), Some(0x41));
        assert_eq!(us_vk_for_char('7'), Some(0x37));
        assert_eq!(us_vk_for_char('['), Some(0xDB));
        assert_eq!(us_vk_for_char('ç'), None);
    }
}
