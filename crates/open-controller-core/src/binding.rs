//! What an extra button does: press an Xbox button, hold a key, or type a short macro. Kept per
//! controller, by its serial when it has one and by model otherwise.

use crate::device::Identity;
use crate::mapping::xusb;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Steps a macro may have, and the longest wait between them.
pub const MAX_STEPS: usize = 32;
pub const MAX_WAIT_MS: u16 = 5000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum XboxButton {
    A,
    B,
    X,
    Y,
    LeftBumper,
    RightBumper,
    LeftTrigger,
    RightTrigger,
    View,
    Menu,
    Guide,
    LeftStick,
    RightStick,
    Up,
    Down,
    Left,
    Right,
}

impl XboxButton {
    pub const ALL: [XboxButton; 17] = [
        XboxButton::A,
        XboxButton::B,
        XboxButton::X,
        XboxButton::Y,
        XboxButton::LeftBumper,
        XboxButton::RightBumper,
        XboxButton::LeftTrigger,
        XboxButton::RightTrigger,
        XboxButton::View,
        XboxButton::Menu,
        XboxButton::Guide,
        XboxButton::LeftStick,
        XboxButton::RightStick,
        XboxButton::Up,
        XboxButton::Down,
        XboxButton::Left,
        XboxButton::Right,
    ];

    pub fn label(self) -> &'static str {
        match self {
            XboxButton::A => "A",
            XboxButton::B => "B",
            XboxButton::X => "X",
            XboxButton::Y => "Y",
            XboxButton::LeftBumper => "LB",
            XboxButton::RightBumper => "RB",
            XboxButton::LeftTrigger => "LT",
            XboxButton::RightTrigger => "RT",
            XboxButton::View => "View",
            XboxButton::Menu => "Menu",
            XboxButton::Guide => "Xbox",
            XboxButton::LeftStick => "LS",
            XboxButton::RightStick => "RS",
            XboxButton::Up => "↑",
            XboxButton::Down => "↓",
            XboxButton::Left => "←",
            XboxButton::Right => "→",
        }
    }

    /// The XUSB button bit, or `None` for a trigger, which is an axis.
    pub fn bit(self) -> Option<u16> {
        Some(match self {
            XboxButton::A => xusb::A,
            XboxButton::B => xusb::B,
            XboxButton::X => xusb::X,
            XboxButton::Y => xusb::Y,
            XboxButton::LeftBumper => xusb::LEFT_SHOULDER,
            XboxButton::RightBumper => xusb::RIGHT_SHOULDER,
            XboxButton::View => xusb::BACK,
            XboxButton::Menu => xusb::START,
            XboxButton::Guide => xusb::GUIDE,
            XboxButton::LeftStick => xusb::LEFT_THUMB,
            XboxButton::RightStick => xusb::RIGHT_THUMB,
            XboxButton::Up => xusb::DPAD_UP,
            XboxButton::Down => xusb::DPAD_DOWN,
            XboxButton::Left => xusb::DPAD_LEFT,
            XboxButton::Right => xusb::DPAD_RIGHT,
            XboxButton::LeftTrigger | XboxButton::RightTrigger => return None,
        })
    }
}

pub mod modifier {
    pub const CTRL: u8 = 1;
    pub const SHIFT: u8 = 2;
    pub const ALT: u8 = 4;
    pub const WIN: u8 = 8;
}

/// A key with the modifiers held with it. `key` is a Windows virtual-key code; 0 is a chord of
/// modifiers only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Chord {
    pub mods: u8,
    pub key: u16,
}

impl Chord {
    pub fn is_empty(&self) -> bool {
        self.mods == 0 && self.key == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Step {
    Keys(Chord),
    Wait(u16),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    /// An Xbox button, held while this one is.
    Xbox(XboxButton),
    /// A key or shortcut, held while this one is.
    Keys(Chord),
    /// Keys typed one after another, once per press.
    Macro(Vec<Step>),
}

impl Action {
    /// Clamped to what is allowed, so a hand-edited settings file cannot ask for a minute-long
    /// macro.
    pub fn sanitised(self) -> Option<Action> {
        match self {
            Action::Keys(c) if c.is_empty() => None,
            Action::Macro(steps) => {
                let steps: Vec<Step> = steps
                    .into_iter()
                    .filter(|s| !matches!(s, Step::Keys(c) if c.is_empty()))
                    .map(|s| match s {
                        Step::Wait(ms) => Step::Wait(ms.min(MAX_WAIT_MS)),
                        k => k,
                    })
                    .take(MAX_STEPS)
                    .collect();
                steps.iter().any(|s| matches!(s, Step::Keys(_))).then_some(Action::Macro(steps))
            }
            a => Some(a),
        }
    }
}

/// One controller's assignments, by SDL button index.
pub type Bindings = BTreeMap<u8, Action>;

/// Where a controller's settings are kept. A serial (the Bluetooth address on most pads) follows
/// one controller across cables and pairings; without one, every controller of the model shares
/// them.
pub fn store_key(identity: &Identity, vendor: u16, product: u16) -> String {
    match identity {
        Identity::Serial(s) => format!("serial:{s}"),
        Identity::Path(_) => format!("model:{vendor:04x}:{product:04x}"),
    }
}

/// The XUSB buttons and triggers the held extra buttons add to a report.
pub fn xbox_overlay(bindings: &Bindings, buttons: u64) -> (u16, bool, bool) {
    let mut bits = 0;
    let (mut lt, mut rt) = (false, false);
    for (&b, action) in bindings {
        if buttons & (1 << b) == 0 {
            continue;
        }
        if let Action::Xbox(x) = action {
            match (x, x.bit()) {
                (_, Some(bit)) => bits |= bit,
                (XboxButton::LeftTrigger, None) => lt = true,
                _ => rt = true,
            }
        }
    }
    (bits, lt, rt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_adds_only_held_buttons() {
        let mut b = Bindings::new();
        b.insert(16, Action::Xbox(XboxButton::A));
        b.insert(17, Action::Xbox(XboxButton::LeftTrigger));
        b.insert(18, Action::Keys(Chord { mods: modifier::CTRL, key: 0x41 }));
        assert_eq!(xbox_overlay(&b, 0), (0, false, false));
        assert_eq!(xbox_overlay(&b, 1 << 16), (xusb::A, false, false));
        assert_eq!(xbox_overlay(&b, 1 << 16 | 1 << 17 | 1 << 18), (xusb::A, true, false));
    }

    #[test]
    fn every_xbox_button_but_the_triggers_has_a_bit() {
        let bits: Vec<u16> = XboxButton::ALL.iter().filter_map(|x| x.bit()).collect();
        assert_eq!(bits.len(), 15);
        assert_eq!(bits.iter().fold(0, |a, b| a | b).count_ones(), 15);
    }

    #[test]
    fn macros_are_clamped() {
        let long = Action::Macro((0..100).map(|_| Step::Wait(60_000)).chain([Step::Keys(Chord { mods: 0, key: 0x41 })]).collect());
        assert_eq!(long.sanitised(), None, "the key fell beyond the step limit");
        let ok = Action::Macro(vec![Step::Keys(Chord { mods: 0, key: 0x41 }), Step::Wait(60_000)]).sanitised();
        assert_eq!(ok, Some(Action::Macro(vec![Step::Keys(Chord { mods: 0, key: 0x41 }), Step::Wait(MAX_WAIT_MS)])));
        assert_eq!(Action::Keys(Chord::default()).sanitised(), None);
    }

    #[test]
    fn store_keys() {
        assert_eq!(store_key(&Identity::Serial("e417d8bc366c".into()), 0x2DC8, 0x6012), "serial:e417d8bc366c");
        assert_eq!(store_key(&Identity::Path("x".into()), 0x2DC8, 0x6012), "model:2dc8:6012");
    }

    #[test]
    fn bindings_round_trip_as_json() {
        let mut b = Bindings::new();
        b.insert(17, Action::Macro(vec![Step::Keys(Chord { mods: modifier::SHIFT, key: 0x31 }), Step::Wait(50)]));
        let text = serde_json::to_string(&b).unwrap();
        assert_eq!(serde_json::from_str::<Bindings>(&text).unwrap(), b);
    }
}
