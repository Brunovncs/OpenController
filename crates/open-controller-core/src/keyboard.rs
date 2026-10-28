//! Keys pressed on the user's behalf for extra buttons, and key names for the interface.
//!
//! Keys go out from a thread of their own, so the input thread never waits on them, and macros
//! play from another, so a macro's waits never hold up a key a button presses or lets go of.
//! The Xbox buttons a macro taps go back to the input thread, which owns the virtual controllers.
//! Chords are Windows virtual-key codes on every system, so settings carry over between them;
//! each system's module turns them into its own key codes.

use crate::binding::{Buttons, Chord, Step, XBOX_TAP_MS, modifier};
use crate::roster::DeviceId;
use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, unbounded};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

#[cfg_attr(windows, path = "keyboard/windows.rs")]
#[cfg_attr(target_os = "linux", path = "keyboard/linux.rs")]
#[cfg_attr(target_os = "macos", path = "keyboard/macos.rs")]
mod sys;

pub use sys::{key_name, vk_for_char};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Out {
    Down(Chord),
    Up(Chord),
}

/// Xbox buttons a macro presses or lets go of, on the virtual controller of the controller whose
/// button played it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tap {
    Down(DeviceId, Buttons),
    Up(DeviceId, Buttons),
}

/// The threads that type. Dropping it stops the macro playing, lets go of its keys, and lets
/// the threads end.
pub struct Keyboard {
    keys: Sender<Out>,
    macros: Sender<(Vec<Step>, DeviceId)>,
    taps: Receiver<Tap>,
    plays: Arc<Mutex<Plays>>,
    /// Never sent on: closed when the keyboard is dropped, which cuts a macro's wait short.
    _stop: Sender<()>,
}

impl Keyboard {
    pub fn start() -> Keyboard {
        let (keys, rx) = unbounded::<Out>();
        std::thread::Builder::new()
            .name("open-controller-keys".into())
            .spawn(move || {
                // The only owner of the sink and of the counts of held keys: macros send their
                // keys here too, so a key a button holds and one a macro taps are counted together.
                // Without a way to type (Linux before the device rule is in place), keys are
                // dropped, and it is tried again with the next one. Opened at once all the same, so
                // the keyboard device is there before the first key.
                let mut sink = sys::Sink::open();
                let mut held = Held::default();
                while let Ok(out) = rx.recv() {
                    if sink.is_none() {
                        sink = sys::Sink::open();
                    }
                    let Some(sink) = sink.as_ref() else { continue };
                    match out {
                        Out::Down(c) => sink.down(held.down(c)),
                        Out::Up(c) => sink.up(held.up(c)),
                    }
                }
            })
            .expect("could not start the keyboard thread");
        let (macros, queued) = unbounded::<(Vec<Step>, DeviceId)>();
        let (to_pads, taps) = unbounded::<Tap>();
        let (stop_tx, stop) = crossbeam_channel::bounded::<()>(0);
        let plays = Arc::new(Mutex::new(Plays::default()));
        let (to_keys, done) = (keys.clone(), plays.clone());
        std::thread::Builder::new()
            .name("open-controller-macros".into())
            .spawn(move || {
                while let Ok((steps, by)) = queued.recv() {
                    play(&to_keys, &to_pads, by, &steps, &stop);
                    done.lock().unwrap_or_else(PoisonError::into_inner).done(&steps);
                }
            })
            .expect("could not start the macro thread");
        Keyboard { keys, macros, taps, plays, _stop: stop_tx }
    }

    /// The Xbox buttons macros press and let go of, for the input thread to read.
    pub fn taps(&self) -> &Receiver<Tap> {
        &self.taps
    }

    pub fn press(&self, c: Chord) {
        let _ = self.keys.send(Out::Down(c));
    }

    pub fn release(&self, c: Chord) {
        let _ = self.keys.send(Out::Up(c));
    }

    /// Plays a macro a button of controller `by` started.
    pub fn play(&self, steps: Vec<Step>, by: DeviceId) {
        if self.plays.lock().unwrap_or_else(PoisonError::into_inner).admit(&steps) {
            let _ = self.macros.send((steps, by));
        }
    }
}

/// Gap between the keys of a macro step, so programs that poll the keyboard see each one.
const TAP: Duration = Duration::from_millis(15);

/// Macros playing or waiting their turn, at most.
const MAX_PLAYS: usize = 4;

/// The macros playing or waiting their turn. A button pressed again while its macro is still
/// playing or waiting is ignored, and only a few wait at all, so pressing a button over and over
/// never queues minutes of typing.
#[derive(Default)]
struct Plays(Vec<Vec<Step>>);

impl Plays {
    /// Whether `steps` is to be played.
    fn admit(&mut self, steps: &[Step]) -> bool {
        if self.0.len() >= MAX_PLAYS || self.0.iter().any(|p| p == steps) {
            return false;
        }
        self.0.push(steps.to_vec());
        true
    }

    fn done(&mut self, steps: &[Step]) {
        if let Some(i) = self.0.iter().position(|p| p == steps) {
            self.0.remove(i);
        }
    }
}

/// Plays `steps` through the key thread, and its Xbox buttons on `by`'s virtual controller. Once
/// `stop` closes it ends early, with every key and button it pressed let go of.
fn play(keys: &Sender<Out>, pads: &Sender<Tap>, by: DeviceId, steps: &[Step], stop: &Receiver<()>) {
    let wait = |d: Duration| !matches!(stop.recv_timeout(d), Err(RecvTimeoutError::Disconnected));
    let xbox_tap = Duration::from_millis(XBOX_TAP_MS.into());
    for s in steps {
        match *s {
            Step::Keys(c) => {
                let _ = keys.send(Out::Down(c));
                let on = wait(TAP);
                let _ = keys.send(Out::Up(c));
                if !on || !wait(TAP) {
                    return;
                }
            }
            Step::Xbox(b) => {
                let _ = pads.send(Tap::Down(by, b));
                let on = wait(xbox_tap);
                let _ = pads.send(Tap::Up(by, b));
                if !on || !wait(xbox_tap) {
                    return;
                }
            }
            Step::Wait(ms) => {
                if !wait(Duration::from_millis(ms.into())) {
                    return;
                }
            }
        }
    }
}

const MODIFIER_BITS: [u8; 4] = [modifier::CTRL, modifier::SHIFT, modifier::ALT, modifier::WIN];

/// How many chords hold each key down, so that letting go of one never lets go of a key another
/// still holds: Ctrl, with two buttons on Ctrl+C and Ctrl+V.
#[derive(Default)]
struct Held {
    mods: [u32; 4],
    keys: HashMap<u16, u32>,
}

impl Held {
    /// The chord to press: all of it, since a key already down stays down when pressed again
    /// and some systems read the modifiers from the key's own event.
    fn down(&mut self, c: Chord) -> Chord {
        for (n, &m) in self.mods.iter_mut().zip(&MODIFIER_BITS) {
            if c.mods & m != 0 {
                *n += 1;
            }
        }
        if c.key != 0 {
            *self.keys.entry(c.key).or_default() += 1;
        }
        c
    }

    /// The part of the chord no other one holds. A key nothing pressed is let go of anyway.
    fn up(&mut self, c: Chord) -> Chord {
        let mut out = Chord { mods: 0, key: 0 };
        for (n, &m) in self.mods.iter_mut().zip(&MODIFIER_BITS) {
            if c.mods & m != 0 {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    out.mods |= m;
                }
            }
        }
        if c.key != 0 {
            let n = self.keys.remove(&c.key).unwrap_or(0).saturating_sub(1);
            if n == 0 {
                out.key = c.key;
            } else {
                self.keys.insert(c.key, n);
            }
        }
        out
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

    #[test]
    fn a_key_two_chords_hold_stays_down_until_both_let_go() {
        use modifier::{CTRL, SHIFT};
        let mut held = Held::default();
        let (copy, paste) = (Chord { mods: CTRL, key: 0x43 }, Chord { mods: CTRL | SHIFT, key: 0x56 });
        assert_eq!(held.down(copy), copy);
        assert_eq!(held.down(paste), paste);
        assert_eq!(held.up(copy), Chord { mods: 0, key: 0x43 }, "Ctrl stays down for the other");
        assert_eq!(held.up(paste), paste);
        // Let go of without having been pressed here: let go of all the same.
        assert_eq!(held.up(copy), copy);
    }

    #[test]
    fn a_macro_pressed_again_while_it_plays_is_not_queued() {
        let (a, b) = (vec![Step::Keys(Chord { mods: 0, key: 0x41 })], vec![Step::Wait(10)]);
        let mut plays = Plays::default();
        assert!(plays.admit(&a));
        assert!(!plays.admit(&a), "already playing");
        assert!(plays.admit(&b));
        plays.done(&a);
        assert!(plays.admit(&a), "played, so it plays again");
        for n in 0..10 {
            plays.admit(&[Step::Wait(n)]);
        }
        assert_eq!(plays.0.len(), MAX_PLAYS, "only a few wait their turn");
    }

    #[test]
    fn a_stopped_macro_lets_go_of_its_keys_at_once() {
        let c = Chord { mods: modifier::CTRL, key: 0x43 };
        let (keys, typed) = unbounded();
        let (stop_tx, stop) = crossbeam_channel::bounded(0);
        let steps = [Step::Keys(c), Step::Wait(5000), Step::Keys(c)];
        let started = std::time::Instant::now();
        let (pads, _) = unbounded();
        let player = std::thread::spawn(move || play(&keys, &pads, 0, &steps, &stop));
        assert_eq!(typed.recv().unwrap(), Out::Down(c));
        drop(stop_tx);
        player.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(2), "the wait was cut short");
        let rest: Vec<Out> = typed.try_iter().collect();
        assert_eq!(rest, [Out::Up(c)]);
    }

    #[test]
    fn a_macro_taps_xbox_buttons_on_the_controller_that_played_it() {
        use crate::binding::XboxButton;
        let (a, lb) = (Buttons::of(&[XboxButton::A]), Buttons::of(&[XboxButton::LeftBumper, XboxButton::Up]));
        let c = Chord { mods: 0, key: 0x41 };
        let (keys, typed) = unbounded();
        let (pads, tapped) = unbounded();
        let (_stop_tx, stop) = crossbeam_channel::bounded(0);
        let started = std::time::Instant::now();
        play(&keys, &pads, 7, &[Step::Xbox(a), Step::Keys(c), Step::Xbox(lb)], &stop);
        let taps: Vec<Tap> = tapped.try_iter().collect();
        assert_eq!(taps, [Tap::Down(7, a), Tap::Up(7, a), Tap::Down(7, lb), Tap::Up(7, lb)]);
        assert_eq!(typed.try_iter().collect::<Vec<Out>>(), [Out::Down(c), Out::Up(c)]);
        assert!(started.elapsed() >= Duration::from_millis(4 * u64::from(XBOX_TAP_MS)), "each button is held, then let go a while");
    }

    #[test]
    fn a_stopped_macro_lets_go_of_its_buttons_at_once() {
        use crate::binding::XboxButton;
        let a = Buttons::of(&[XboxButton::A]);
        let (keys, _) = unbounded();
        let (pads, tapped) = unbounded();
        let (stop_tx, stop) = crossbeam_channel::bounded(0);
        let player = std::thread::spawn(move || play(&keys, &pads, 3, &[Step::Xbox(a), Step::Wait(5000)], &stop));
        assert_eq!(tapped.recv().unwrap(), Tap::Down(3, a));
        drop(stop_tx);
        player.join().unwrap();
        assert_eq!(tapped.try_iter().collect::<Vec<Tap>>(), [Tap::Up(3, a)]);
    }
}
