//! Incoming controller chords, distinct from the Xbox buttons an assignment outputs.
//! Triggers use the two high bits; real SDL/extra buttons occupy bits 0..=40.
use crate::binding::{self, Action, Bindings, Chord};
use crate::mapping::{PadState, axis};
use std::collections::BTreeMap;

pub type Shortcuts = BTreeMap<u64, Chord>;
pub const LT: u64 = 1 << 62;
pub const RT: u64 = 1 << 63;
pub const TRIGGERS: u64 = LT | RT;
pub const MAX_SHORTCUTS: usize = 32;
pub const MAX_NAME: usize = 64;
pub const MAX_UNASSIGNED: usize = 256;
pub const DEFAULT_HOLD_MS: u16 = 100;
pub const MIN_HOLD_MS: u16 = 10;
pub const MAX_HOLD_MS: u16 = 60_000;
pub fn default_hold_ms() -> u16 {
    DEFAULT_HOLD_MS
}
pub fn hold_ms(ms: u16) -> u16 {
    ms.clamp(MIN_HOLD_MS, MAX_HOLD_MS)
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShortcutAction {
    #[serde(default)]
    pub hold_until_release: bool,
    #[serde(default = "default_hold_ms")]
    pub hold_ms: u16,
    pub name: String,
    pub keys: Chord,
    pub anchor: u8,
}

/// An optional human-readable name, kept independently from input and output bindings.
pub fn clean_name(name: &str) -> String {
    name.trim().chars().filter(|c| !c.is_control()).take(MAX_NAME).collect()
}

pub fn input(s: &PadState) -> u64 {
    (s.buttons & !TRIGGERS)
        | if s.axes[axis::LEFT_TRIGGER] > 16383 { LT } else { 0 }
        | if s.axes[axis::RIGHT_TRIGGER] > 16383 { RT } else { 0 }
}

pub fn valid(mask: u64, keys: Chord) -> bool {
    mask != 0 && !keys.is_empty() && keys.mods & !15 == 0 && keys.key <= 255
}

/// Extra buttons used as chord modifiers act on release when tapped alone. Normal buttons
/// remain immediate when played alone, and are consumed only after a complete chord matches.
pub fn deferred<'a>(shortcuts: impl Iterator<Item = (&'a u64, &'a Chord)>) -> u64 {
    let normal = binding::NORMAL.iter().fold(0, |mask, &b| mask | 1 << b);
    let entries: Vec<u64> = shortcuts.map(|(&b, _)| b).collect();
    let singles = entries.iter().filter(|b| b.count_ones() == 1).fold(0, |m, b| m | b);
    (entries.iter().fold(0, |m, b| m | b) & !normal & !TRIGGERS) | singles
}

pub fn suppress(s: &mut PadState, mask: u64) {
    s.buttons &= !mask;
    for (bit, axis) in [(LT, axis::LEFT_TRIGGER), (RT, axis::RIGHT_TRIGGER)] {
        if mask & bit != 0 {
            s.axes[axis] = 0;
        }
    }
}

#[derive(Default)]
pub struct State {
    /// Consumed components stay consumed until each is released: releasing Capture first
    /// must not leak the still-held A into a game or run A's keyboard assignment.
    pub used: u64,
    pending: u64,
    held_outputs: BTreeMap<u64, Chord>,
}

pub struct Transition {
    pub presses: Vec<Chord>,
    pub releases: Vec<Chord>,
    pub before: u64,
    pub after: u64,
    pub fire: Option<Chord>,
    pub fire_input: Option<u64>,
    pub taps: Vec<Action>,
    pub shortcut_taps: Vec<(u64, Chord)>,
}

impl State {
    pub fn update_modes<'a>(
        &mut self,
        shortcuts: impl Iterator<Item = (&'a u64, &'a Chord)> + Clone,
        bindings: &Bindings,
        held_inputs: &std::collections::BTreeSet<u64>,
        before: u64,
        after: u64,
    ) -> Transition {
        let delay = deferred(shortcuts.clone());
        let visible_before = before & !self.used & !delay & !TRIGGERS;
        let released = self.pending & !after & !self.used;
        let shortcut_taps = shortcuts
            .clone()
            .filter(|(mask, _)| mask.count_ones() == 1 && !held_inputs.contains(mask) && released & **mask != 0)
            .map(|(&mask, &c)| (mask, c))
            .collect();
        let taps = bindings
            .iter()
            .filter(|(b, _)| released & (1u64 << **b) != 0 && !shortcuts.clone().any(|(&m, _)| m == 1u64 << **b))
            .map(|(_, a)| a.clone())
            .collect();
        self.used &= after;
        self.pending = (self.pending & after) | (after & !before & delay);
        // Longest chord wins if several match the same report; stable order breaks ties.
        let matched = shortcuts
            .clone()
            .filter(|(mask, _)| {
                after & **mask == **mask
                    && (mask.count_ones() > 1
                        || held_inputs.contains(mask) && (before & **mask != **mask || self.held_outputs.contains_key(mask)))
            })
            .max_by_key(|(mask, _)| (mask.count_ones(), std::cmp::Reverse(**mask)));
        let mut releases = Vec::new();
        self.held_outputs.retain(|mask, chord| {
            let superseded = matched.is_some_and(|(&other, _)| other != *mask && other & *mask == *mask);
            let keep = after & *mask == *mask
                && held_inputs.contains(mask)
                && shortcuts.clone().any(|(&m, &c)| m == *mask && c == *chord)
                && !superseded;
            if !keep {
                releases.push(*chord);
            }
            keep
        });
        let mut presses = Vec::new();
        let fire = matched.and_then(|(&mask, &chord)| {
            self.used |= mask;
            self.pending &= !mask;
            // No repeat while held, nor a shorter chord firing when a longer one is released.
            let starts = before & mask != mask && after & !before & mask != 0;
            if held_inputs.contains(&mask) {
                if starts && !self.held_outputs.contains_key(&mask) {
                    self.held_outputs.insert(mask, chord);
                    presses.push(chord);
                }
                None
            } else {
                starts.then_some(chord)
            }
        });
        let fire_input = matched.filter(|_| fire.is_some()).map(|(&mask, _)| mask);
        Transition {
            presses,
            releases,
            fire_input,
            before: visible_before,
            after: after & !self.used & !delay & !TRIGGERS,
            fire,
            taps,
            shortcut_taps,
        }
    }
    pub fn release_all(&mut self) -> Vec<Chord> {
        std::mem::take(&mut self.held_outputs).into_values().collect()
    }

    #[cfg(test)]
    pub fn update<'a>(
        &mut self,
        shortcuts: impl Iterator<Item = (&'a u64, &'a Chord)> + Clone,
        bindings: &Bindings,
        before: u64,
        after: u64,
    ) -> Transition {
        self.update_modes(shortcuts, bindings, &Default::default(), before, after)
    }
}

/// A recorder keeps buttons that were held together, not a sequence of unrelated taps.
/// Buttons already down when it opens must first be released.
#[derive(Default)]
pub struct Recorder {
    ready: bool,
    peak: u64,
    pub shown: u64,
}

impl Recorder {
    pub fn new(held: u64) -> Self {
        Self { ready: held == 0, ..Self::default() }
    }
    pub fn feed(&mut self, held: u64) -> Option<u64> {
        if !self.ready {
            self.ready = held == 0;
            return None;
        }
        if held != 0 {
            self.shown = held;
            if held.count_ones() >= self.peak.count_ones() {
                self.peak = held;
            }
            None
        } else if self.peak != 0 {
            let result = std::mem::take(&mut self.peak);
            self.shown = result;
            Some(result)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const CAPTURE: u64 = 1 << 15;
    const A: u64 = 1;
    const B: u64 = 2;
    const KEY: Chord = Chord::new(5, 121);
    fn setup() -> (Shortcuts, Bindings, State) {
        ([(CAPTURE | A, KEY)].into(), [(15, Action::Keys(Chord::new(0, 121)))].into(), State::default())
    }
    #[test]
    fn chord_fires_once_and_consumes_components_until_both_released() {
        let (s, b, mut r) = setup();
        let out = r.update(s.iter(), &b, 0, CAPTURE | A);
        assert_eq!(out.fire, Some(KEY));
        assert_eq!(out.after, 0);
        assert!(out.taps.is_empty());
        assert_eq!(r.update(s.iter(), &b, CAPTURE | A, CAPTURE | A).fire, None);
        let out = r.update(s.iter(), &b, CAPTURE | A, A);
        assert_eq!(out.after, 0);
        assert_eq!(out.fire, None);
        assert!(out.taps.is_empty());
        assert!(r.update(s.iter(), &b, A, 0).taps.is_empty());
        assert_eq!(r.used, 0);
        assert_eq!(r.update(s.iter(), &b, 0, A).after, A);
    }
    #[test]
    fn capture_alone_taps_its_original_action_on_release() {
        let (s, b, mut r) = setup();
        assert_eq!(r.update(s.iter(), &b, 0, CAPTURE).after, 0);
        assert_eq!(r.update(s.iter(), &b, CAPTURE, 0).taps, vec![b[&15].clone()]);
    }
    #[test]
    fn sequential_modifier_does_not_run_capture_and_can_repeat_with_a_held_modifier() {
        let (s, b, mut r) = setup();
        r.update(s.iter(), &b, 0, CAPTURE);
        assert_eq!(r.update(s.iter(), &b, CAPTURE, CAPTURE | A).fire, Some(KEY));
        r.update(s.iter(), &b, CAPTURE | A, CAPTURE);
        assert_eq!(r.update(s.iter(), &b, CAPTURE, CAPTURE | A).fire, Some(KEY));
        assert!(r.update(s.iter(), &b, CAPTURE | A, 0).taps.is_empty());
    }
    #[test]
    fn longest_chord_wins_and_release_does_not_trigger_the_shorter_one() {
        let (mut s, b, mut r) = setup();
        let other = Chord::new(0, 122);
        s.insert(CAPTURE | A | B, other);
        assert_eq!(r.update(s.iter(), &b, 0, CAPTURE | A | B).fire, Some(other));
        assert_eq!(r.update(s.iter(), &b, CAPTURE | A | B, CAPTURE | A).fire, None);
    }
    #[test]
    fn triggers_participate_in_chords_and_are_suppressed_from_the_game() {
        let mut p = PadState { buttons: CAPTURE, ..Default::default() };
        p.axes[axis::LEFT_TRIGGER] = 20000;
        assert_eq!(input(&p), CAPTURE | LT);
        let s: Shortcuts = [(CAPTURE | LT, KEY)].into();
        let mut r = State::default();
        assert_eq!(r.update(s.iter(), &Bindings::new(), 0, input(&p)).fire, Some(KEY));
        suppress(&mut p, r.used);
        assert_eq!(p.buttons, 0);
        assert_eq!(p.axes[axis::LEFT_TRIGGER], 0);
    }
    #[test]
    fn record_release_and_reject_preheld_and_sequential_taps() {
        let mut r = Recorder::new(CAPTURE);
        assert_eq!(r.feed(CAPTURE | A), None);
        assert_eq!(r.feed(0), None);
        assert_eq!(r.feed(CAPTURE), None);
        assert_eq!(r.feed(CAPTURE | A), None);
        assert_eq!(r.feed(CAPTURE), None);
        assert_eq!(r.feed(0), Some(CAPTURE | A));
        assert_eq!(r.feed(A), None);
        assert_eq!(r.feed(0), Some(A));
        assert_eq!(r.feed(B), None);
        assert_eq!(r.feed(0), Some(B));
    }
    #[test]
    fn single_capture_fires_on_release_and_never_after_a_completed_combination() {
        let (mut shortcuts, bindings, mut state) = setup();
        let single = Chord::new(0, 122);
        shortcuts.insert(CAPTURE, single);
        assert_eq!(state.update(shortcuts.iter(), &bindings, 0, CAPTURE).fire, None);
        let out = state.update(shortcuts.iter(), &bindings, CAPTURE, 0);
        assert_eq!(out.shortcut_taps, [(CAPTURE, single)]);
        state.update(shortcuts.iter(), &bindings, 0, CAPTURE);
        let out = state.update(shortcuts.iter(), &bindings, CAPTURE, CAPTURE | A);
        assert_eq!(out.fire, Some(KEY));
        assert!(out.taps.is_empty());
        assert!(state.update(shortcuts.iter(), &bindings, CAPTURE | A, A).shortcut_taps.is_empty());
        assert!(state.update(shortcuts.iter(), &bindings, A, 0).shortcut_taps.is_empty());
        assert!(state.update(shortcuts.iter(), &bindings, 0, CAPTURE).taps.is_empty());
        assert_eq!(state.update(shortcuts.iter(), &bindings, CAPTURE, 0).shortcut_taps, [(CAPTURE, single)]);
    }
    #[test]
    fn held_chord_presses_once_and_releases_when_any_component_breaks() {
        let (s, b, mut state) = setup();
        let held = [CAPTURE | A].into();
        let out = state.update_modes(s.iter(), &b, &held, 0, CAPTURE | A);
        assert_eq!(out.presses, [KEY]);
        assert!(out.fire.is_none());
        let out = state.update_modes(s.iter(), &b, &held, CAPTURE | A, CAPTURE | A);
        assert!(out.presses.is_empty() && out.releases.is_empty());
        let out = state.update_modes(s.iter(), &b, &held, CAPTURE | A, A);
        assert_eq!(out.releases, [KEY]);
        assert_eq!(out.after, 0);
        assert!(state.update_modes(s.iter(), &b, &held, A, 0).releases.is_empty());
    }
    #[test]
    fn held_single_starts_immediately_and_a_larger_chord_replaces_it_without_restart() {
        let single = Chord::new(4, 9);
        let (mut s, b, mut state) = setup();
        s.insert(CAPTURE, single);
        let held = [CAPTURE, CAPTURE | A].into();
        assert_eq!(state.update_modes(s.iter(), &b, &held, 0, CAPTURE).presses, [single]);
        let out = state.update_modes(s.iter(), &b, &held, CAPTURE, CAPTURE | A);
        assert_eq!(out.releases, [single]);
        assert_eq!(out.presses, [KEY]);
        let out = state.update_modes(s.iter(), &b, &held, CAPTURE | A, CAPTURE);
        assert_eq!(out.releases, [KEY]);
        assert!(out.presses.is_empty());
        assert!(state.update_modes(s.iter(), &b, &held, CAPTURE, 0).shortcut_taps.is_empty());
    }
    #[test]
    fn held_outputs_can_be_drained_on_profile_change_or_disconnect() {
        let (s, b, mut state) = setup();
        let held = [CAPTURE | A].into();
        state.update_modes(s.iter(), &b, &held, 0, CAPTURE | A);
        assert_eq!(state.release_all(), [KEY]);
        assert!(state.release_all().is_empty());
    }
}
