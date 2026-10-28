//! What a button does: press Xbox buttons, hold a key, or play a short macro. Kept per
//! controller, by its serial when it has one and by model otherwise. An extra button does nothing
//! else; any other button given something to do stops being itself.

use crate::device::Identity;
use crate::extras;
use crate::mapping::{PadState, XusbReport, button, to_xusb, xusb};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Steps a macro may have, and the longest wait between them.
pub const MAX_STEPS: usize = 32;
pub const MAX_WAIT_MS: u16 = 5000;

/// How long a macro holds Xbox buttons, and then waits with them let go: games read the
/// controller once a frame, so a shorter tap can fall between two reads.
pub const XBOX_TAP_MS: u16 = 50;

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

/// Xbox buttons pressed together, a bit each in [`XboxButton::ALL`]'s order. Written as a list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "Vec<XboxButton>", into = "Vec<XboxButton>")]
pub struct Buttons(u32);

impl Buttons {
    pub fn of(list: &[XboxButton]) -> Buttons {
        Buttons(list.iter().fold(0, |m, &x| m | 1 << x as u32))
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    pub fn contains(self, x: XboxButton) -> bool {
        self.0 & 1 << x as u32 != 0
    }

    pub fn toggled(self, x: XboxButton) -> Buttons {
        Buttons(self.0 ^ 1 << x as u32)
    }

    pub fn with(self, other: Buttons) -> Buttons {
        Buttons(self.0 | other.0)
    }

    /// The buttons in this one and not in `other`.
    pub fn without(self, other: Buttons) -> Buttons {
        Buttons(self.0 & !other.0)
    }

    pub fn iter(self) -> impl Iterator<Item = XboxButton> {
        XboxButton::ALL.into_iter().filter(move |&x| self.contains(x))
    }

    /// Presses them in a report; a trigger is pulled all the way.
    pub fn press(self, r: &mut XusbReport) {
        for x in self.iter() {
            match (x, x.bit()) {
                (_, Some(bit)) => r.buttons |= bit,
                (XboxButton::LeftTrigger, None) => r.left_trigger = u8::MAX,
                _ => r.right_trigger = u8::MAX,
            }
        }
    }

    /// The buttons a report presses, a trigger once it is past half way.
    pub fn in_report(r: &XusbReport) -> Buttons {
        let pulled = |t: u8| t > u8::MAX / 2;
        let list: Vec<XboxButton> = XboxButton::ALL
            .into_iter()
            .filter(|&x| match (x, x.bit()) {
                (_, Some(bit)) => r.buttons & bit != 0,
                (XboxButton::LeftTrigger, None) => pulled(r.left_trigger),
                _ => pulled(r.right_trigger),
            })
            .collect();
        Buttons::of(&list)
    }
}

impl From<Vec<XboxButton>> for Buttons {
    fn from(list: Vec<XboxButton>) -> Buttons {
        Buttons::of(&list)
    }
}

impl From<Buttons> for Vec<XboxButton> {
    fn from(b: Buttons) -> Vec<XboxButton> {
        b.iter().collect()
    }
}

impl std::fmt::Display for Buttons {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let labels: Vec<&str> = self.iter().map(XboxButton::label).collect();
        f.write_str(&labels.join(" + "))
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
    /// Xbox buttons tapped together, for [`XBOX_TAP_MS`].
    Xbox(Buttons),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    /// An Xbox button, held while this one is.
    Xbox(XboxButton),
    /// A key or shortcut, held while this one is.
    Keys(Chord),
    /// Keys and Xbox buttons one after another, once per press.
    Macro(Vec<Step>),
    /// Several Xbox buttons, held together while this one is.
    Combo(Buttons),
}

impl Action {
    /// Clamped to what is allowed, so a hand-edited settings file cannot ask for a minute-long
    /// macro. A combo of one button is that button.
    pub fn sanitised(self) -> Option<Action> {
        match self {
            Action::Keys(c) if c.is_empty() => None,
            Action::Macro(steps) => {
                let steps: Vec<Step> = steps
                    .into_iter()
                    .filter(|s| match s {
                        Step::Keys(c) => !c.is_empty(),
                        Step::Xbox(b) => !b.is_empty(),
                        Step::Wait(_) => true,
                    })
                    .map(|s| match s {
                        Step::Wait(ms) => Step::Wait(ms.min(MAX_WAIT_MS)),
                        k => k,
                    })
                    .take(MAX_STEPS)
                    .collect();
                steps.iter().any(|s| !matches!(s, Step::Wait(_))).then_some(Action::Macro(steps))
            }
            Action::Combo(b) if b.len() <= 1 => b.iter().next().map(Action::Xbox),
            a => Some(a),
        }
    }

    /// The Xbox buttons it holds while its button is held, if that is what it does.
    pub fn xbox(&self) -> Option<Buttons> {
        match self {
            Action::Xbox(x) => Some(Buttons::of(&[*x])),
            Action::Combo(b) => Some(*b),
            _ => None,
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
        Identity::Path(_) | Identity::Pair(..) => format!("model:{vendor:04x}:{product:04x}"),
    }
}

/// The buttons every gamepad has, by SDL index, in the order they are listed: the face buttons,
/// the bumpers, View, Menu and Guide, the stick clicks and the d-pad. Games see each as the Xbox
/// button in its place until it is given something else to do.
pub const NORMAL: [u8; 15] = [
    button::SOUTH as u8,
    button::EAST as u8,
    button::WEST as u8,
    button::NORTH as u8,
    button::LEFT_SHOULDER as u8,
    button::RIGHT_SHOULDER as u8,
    button::BACK as u8,
    button::START as u8,
    button::GUIDE as u8,
    button::LEFT_STICK as u8,
    button::RIGHT_STICK as u8,
    button::DPAD_UP as u8,
    button::DPAD_DOWN as u8,
    button::DPAD_LEFT as u8,
    button::DPAD_RIGHT as u8,
];

pub fn is_normal(b: u8) -> bool {
    NORMAL.contains(&b)
}

fn bit(b: u8) -> u64 {
    1u64.checked_shl(u32::from(b)).unwrap_or(0)
}

/// The buttons that have something to do, which games no longer see as themselves.
pub fn taken(bindings: &Bindings) -> u64 {
    bindings.keys().fold(0, |m, &b| m | bit(b))
}

/// Whether any of the buttons every gamepad has does something else.
pub fn remaps_normal(bindings: &Bindings) -> bool {
    bindings.keys().any(|&b| is_normal(b))
}

/// Presses the Xbox buttons and triggers the held buttons stand for. `buttons` is what is held,
/// before the buttons that do something else are taken out of the report.
pub fn overlay(bindings: &Bindings, buttons: u64, r: &mut XusbReport) {
    for (&b, action) in bindings {
        if buttons & bit(b) != 0
            && let Some(x) = action.xbox()
        {
            x.press(r);
        }
    }
}

/// The Xbox buttons games see pressed for a controller's state, through its assignments; a
/// button that types keys presses none.
pub fn xbox_pressed(bindings: &Bindings, s: &PadState) -> Buttons {
    let mut r = to_xusb(&PadState { buttons: s.buttons & !taken(bindings), ..*s });
    overlay(bindings, s.buttons, &mut r);
    Buttons::in_report(&r)
}

/// Whether a virtual DualShock 4's touchpad is clicked: the controller's is, and no button the
/// click makes (the touchpad's, its halves') does something else.
pub fn touchpad_click(bindings: &Bindings, buttons: u64) -> bool {
    let click = [extras::TOUCHPAD, extras::TOUCH_LEFT, extras::TOUCH_RIGHT, extras::TOUCH_TWO].iter().fold(0, |m, &b| m | bit(b));
    buttons & bit(extras::TOUCHPAD) != 0 && buttons & click & taken(bindings) == 0
}

/// SDL names the face buttons by where they are, so a Nintendo controller's B, at the bottom, is
/// Xbox's A. These make each one the Xbox button of the letter printed on it.
pub fn letters() -> [(u8, Action); 4] {
    [
        (button::SOUTH as u8, Action::Xbox(XboxButton::B)),
        (button::EAST as u8, Action::Xbox(XboxButton::A)),
        (button::WEST as u8, Action::Xbox(XboxButton::Y)),
        (button::NORTH as u8, Action::Xbox(XboxButton::X)),
    ]
}

pub fn uses_letters(bindings: &Bindings) -> bool {
    letters().iter().all(|(b, a)| bindings.get(b) == Some(a))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::mapping::axis;
    use crate::motion;
    use crate::profile::DpadStick;

    /// What the engine sends for `held`: the buttons that do something else taken out, then
    /// what they stand for added.
    fn report(b: &Bindings, held: &[u32]) -> XusbReport {
        let buttons = held.iter().fold(0, |m, &x| m | 1 << x);
        let mut r = to_xusb(&PadState { buttons: buttons & !taken(b), ..PadState::default() });
        overlay(b, buttons, &mut r);
        r
    }

    #[test]
    fn overlay_adds_only_held_buttons() {
        let mut b = Bindings::new();
        b.insert(16, Action::Xbox(XboxButton::A));
        b.insert(17, Action::Xbox(XboxButton::LeftTrigger));
        b.insert(18, Action::Keys(Chord { mods: modifier::CTRL, key: 0x41 }));
        assert_eq!(report(&b, &[]), XusbReport::default());
        assert_eq!(report(&b, &[16]).buttons, xusb::A);
        let r = report(&b, &[16, 17, 18]);
        assert_eq!((r.buttons, r.left_trigger, r.right_trigger), (xusb::A, 255, 0));
    }

    #[test]
    fn a_combo_presses_every_button_at_once() {
        let mut b = Bindings::new();
        b.insert(button::RIGHT_STICK as u8, Action::Combo(Buttons::of(&[XboxButton::A, XboxButton::B, XboxButton::RightTrigger])));
        let r = report(&b, &[button::RIGHT_STICK]);
        assert_eq!((r.buttons, r.right_trigger), (xusb::A | xusb::B, 255), "the stick click itself is gone");
        assert_eq!(report(&b, &[]), XusbReport::default());
    }

    #[test]
    fn combos_of_one_or_none_are_simpler() {
        assert_eq!(Action::Combo(Buttons::of(&[XboxButton::Y])).sanitised(), Some(Action::Xbox(XboxButton::Y)));
        assert_eq!(Action::Combo(Buttons::default()).sanitised(), None);
        let two = Action::Combo(Buttons::of(&[XboxButton::Y, XboxButton::X]));
        assert_eq!(two.clone().sanitised(), Some(two));
    }

    #[test]
    fn buttons_are_written_as_a_list() {
        let b = Buttons::of(&[XboxButton::B, XboxButton::A]);
        assert_eq!(serde_json::to_string(&Action::Combo(b)).unwrap(), r#"{"Combo":["A","B"]}"#);
        assert_eq!(serde_json::from_str::<Buttons>(r#"["B","A","A"]"#).unwrap(), b);
        assert_eq!(b.to_string(), "A + B");
        assert!(b.contains(XboxButton::A) && !b.contains(XboxButton::X) && b.len() == 2);
        assert_eq!(b.toggled(XboxButton::A), Buttons::of(&[XboxButton::B]));
        assert_eq!(b.without(Buttons::of(&[XboxButton::B, XboxButton::Y])), Buttons::of(&[XboxButton::A]));
    }

    #[test]
    fn what_games_see_pressed() {
        let mut b = Bindings::new();
        b.insert(button::SOUTH as u8, Action::Xbox(XboxButton::B));
        b.insert(button::BACK as u8, Action::Keys(Chord { mods: 0, key: 0x4D }));
        b.insert(extras::LEFT_PADDLE1, Action::Combo(Buttons::of(&[XboxButton::X, XboxButton::Y])));
        let held = |list: &[u32], lt: i16| {
            let mut s = PadState { buttons: list.iter().fold(0, |m, &x| m | 1 << x), ..PadState::default() };
            s.axes[axis::LEFT_TRIGGER] = lt;
            s
        };
        assert_eq!(xbox_pressed(&b, &held(&[button::SOUTH, button::NORTH], 0)), Buttons::of(&[XboxButton::B, XboxButton::Y]));
        assert_eq!(xbox_pressed(&b, &held(&[button::BACK], 0)), Buttons::default(), "a key is no Xbox button");
        assert_eq!(xbox_pressed(&b, &held(&[u32::from(extras::LEFT_PADDLE1)], 0)), Buttons::of(&[XboxButton::X, XboxButton::Y]));
        assert_eq!(xbox_pressed(&b, &held(&[], 20000)), Buttons::of(&[XboxButton::LeftTrigger]));
        assert_eq!(xbox_pressed(&b, &held(&[], 9000)), Buttons::default(), "a trigger barely pulled");
    }

    #[test]
    fn a_button_given_another_is_no_longer_itself() {
        let mut b = Bindings::new();
        b.insert(button::SOUTH as u8, Action::Xbox(XboxButton::B));
        assert_eq!(report(&b, &[button::SOUTH]).buttons, xusb::B, "A is gone, B is pressed");
        assert_eq!(report(&b, &[button::EAST]).buttons, xusb::B, "B itself still works");
        assert_eq!(report(&b, &[button::NORTH]).buttons, xusb::Y, "the others are untouched");
    }

    #[test]
    fn a_button_can_become_a_trigger() {
        let mut b = Bindings::new();
        b.insert(button::LEFT_SHOULDER as u8, Action::Xbox(XboxButton::LeftTrigger));
        let r = report(&b, &[button::LEFT_SHOULDER]);
        assert_eq!((r.buttons, r.left_trigger), (0, 255));
    }

    #[test]
    fn a_button_given_a_key_leaves_the_game() {
        let mut b = Bindings::new();
        b.insert(button::BACK as u8, Action::Keys(Chord { mods: 0, key: 0x4D }));
        assert_eq!(report(&b, &[button::BACK]), XusbReport::default());
        b.insert(button::START as u8, Action::Macro(vec![Step::Keys(Chord { mods: 0, key: 0x41 })]));
        assert_eq!(report(&b, &[button::START, button::SOUTH]).buttons, xusb::A);
    }

    #[test]
    fn two_buttons_can_trade() {
        let mut b = Bindings::new();
        b.insert(button::SOUTH as u8, Action::Xbox(XboxButton::B));
        b.insert(button::EAST as u8, Action::Xbox(XboxButton::A));
        assert_eq!(report(&b, &[button::SOUTH]).buttons, xusb::B);
        assert_eq!(report(&b, &[button::EAST]).buttons, xusb::A);
        assert_eq!(report(&b, &[button::SOUTH, button::EAST]).buttons, xusb::A | xusb::B);
    }

    #[test]
    fn a_dpad_from_the_stick_is_not_taken_with_the_button() {
        let mut b = Bindings::new();
        b.insert(button::DPAD_UP as u8, Action::Xbox(XboxButton::Y));
        let held = 1 << button::DPAD_UP;
        let mut s = PadState { buttons: held & !taken(&b), ..PadState::default() };
        s.axes[axis::LEFT_Y] = -32000;
        motion::apply_dpad_stick(&mut s, DpadStick::Left);
        let mut r = to_xusb(&s);
        overlay(&b, held, &mut r);
        assert_eq!(r.buttons, xusb::DPAD_UP | xusb::Y, "the stick still pushes up, the button is Y");
    }

    #[test]
    fn the_letters_follow_what_is_printed() {
        let mut b = Bindings::new();
        assert!(!uses_letters(&b));
        b.extend(letters());
        assert!(uses_letters(&b) && remaps_normal(&b));
        assert_eq!(report(&b, &[button::SOUTH]).buttons, xusb::B, "Nintendo's B, at the bottom");
        assert_eq!(report(&b, &[button::EAST]).buttons, xusb::A);
        assert_eq!(report(&b, &[button::WEST]).buttons, xusb::Y);
        assert_eq!(report(&b, &[button::NORTH]).buttons, xusb::X);
    }

    #[test]
    fn extras_are_not_normal_buttons() {
        assert_eq!(NORMAL.iter().fold(0u64, |m, &b| m | bit(b)), (1 << 15) - 1, "SDL's buttons 0 to 14");
        assert!(extras::ALL.iter().all(|&b| !is_normal(b)));
        let mut b = Bindings::new();
        b.insert(extras::LEFT_PADDLE1, Action::Xbox(XboxButton::A));
        assert!(!remaps_normal(&b));
        assert_eq!(taken(&[(200, Action::Xbox(XboxButton::A))].into()), 0, "a button past the u64 is ignored");
    }

    #[test]
    fn a_touchpad_click_that_does_something_else_does_not_click() {
        let click = 1 << extras::TOUCHPAD | 1 << extras::TOUCH_LEFT;
        let mut b = Bindings::new();
        assert!(touchpad_click(&b, click));
        b.insert(extras::TOUCH_RIGHT, Action::Xbox(XboxButton::X));
        assert!(touchpad_click(&b, click), "the other half");
        b.insert(extras::TOUCH_LEFT, Action::Xbox(XboxButton::X));
        assert!(!touchpad_click(&b, click));
        assert!(!touchpad_click(&Bindings::new(), 0));
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
        let a = Step::Xbox(Buttons::of(&[XboxButton::A]));
        let pad = Action::Macro(vec![a, Step::Wait(20), Step::Xbox(Buttons::default())]);
        assert_eq!(pad.sanitised(), Some(Action::Macro(vec![a, Step::Wait(20)])), "Xbox buttons alone make a macro");
        assert_eq!(Action::Macro(vec![Step::Wait(20), Step::Xbox(Buttons::default())]).sanitised(), None);
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
        b.insert(18, Action::Macro(vec![Step::Xbox(Buttons::of(&[XboxButton::LeftBumper, XboxButton::Up]))]));
        b.insert(19, Action::Combo(Buttons::of(&[XboxButton::A, XboxButton::Guide])));
        let text = serde_json::to_string(&b).unwrap();
        assert_eq!(serde_json::from_str::<Bindings>(&text).unwrap(), b);
    }
}
