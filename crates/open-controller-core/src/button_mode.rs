//! Each editor tab is an exclusive button mode. Inactive assignments are remembered so a
//! user can switch Capture from F10 to controller combinations and back without resetting it.
use crate::binding::{self, Action, Buttons, Chord, Step};
use crate::profile::Profile;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Xbox,
    Key,
    Macro,
    Combinations,
    Nothing,
}

impl Mode {
    pub fn of(action: &Action) -> Self {
        match action {
            Action::Xbox(_) | Action::Combo(_) => Self::Xbox,
            Action::Keys(_) => Self::Key,
            Action::Macro(_) => Self::Macro,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Presets {
    pub xbox: Option<Buttons>,
    pub key: Option<Chord>,
    pub macro_steps: Vec<Step>,
}

impl Presets {
    fn remember(&mut self, action: &Action) {
        match action {
            Action::Xbox(_) | Action::Combo(_) => self.xbox = action.xbox(),
            Action::Keys(c) => self.key = Some(*c),
            Action::Macro(steps) => self.macro_steps = steps.clone(),
        }
    }

    fn clear(&mut self, mode: Mode) {
        match mode {
            Mode::Xbox => self.xbox = None,
            Mode::Key => self.key = None,
            Mode::Macro => self.macro_steps.clear(),
            _ => {}
        }
    }

    fn action(&self, mode: Mode) -> Option<Action> {
        match mode {
            Mode::Xbox => self.xbox.map(Action::Combo),
            Mode::Key => self.key.map(Action::Keys),
            Mode::Macro => Some(Action::Macro(self.macro_steps.clone())),
            _ => None,
        }
        .and_then(Action::sanitised)
    }

    fn sanitise(&mut self) {
        self.xbox = self.xbox.filter(|b| !b.is_empty());
        self.key = self.key.filter(|c| !c.is_empty() && c.mods & !15 == 0 && c.key <= 255);
        self.macro_steps = match Action::Macro(std::mem::take(&mut self.macro_steps)).sanitised() {
            Some(Action::Macro(steps)) => steps,
            _ => Vec::new(),
        };
    }
}

impl Profile {
    pub fn button_mode(&self, button: u8) -> Mode {
        self.button_modes.get(&button).copied().unwrap_or_else(|| {
            self.bindings.get(&button).map(Mode::of).unwrap_or(if binding::is_normal(button) { Mode::Nothing } else { Mode::Xbox })
        })
    }

    pub fn assign_button(&mut self, button: u8, action: Option<Action>) {
        if button >= 62 {
            return;
        }
        match action.and_then(Action::sanitised) {
            Some(action) => {
                self.button_modes.insert(button, Mode::of(&action));
                self.button_presets.entry(button).or_default().remember(&action);
                self.bindings.insert(button, action);
            }
            None => {
                let mode = self.button_mode(button);
                if let Some(p) = self.button_presets.get_mut(&button) {
                    p.clear(mode);
                }
                self.bindings.remove(&button);
            }
        }
    }

    pub fn select_button_mode(&mut self, button: u8, mode: Mode) {
        if button >= 62 {
            return;
        }
        if let Some(action) = self.bindings.remove(&button) {
            self.button_presets.entry(button).or_default().remember(&action);
        }
        self.button_modes.insert(button, mode);
        if let Some(action) = self.button_presets.get(&button).and_then(|p| p.action(mode)) {
            self.bindings.insert(button, action);
        }
    }

    pub fn shortcut_anchor(&self, input: u64) -> u8 {
        self.shortcut_anchors.get(&input).copied().unwrap_or_else(|| {
            // Migrate the earlier flat editor: prefer Capture, then an extra, then a normal
            // button. Legacy trigger-only shortcuts remain accessible under Capture.
            if input & (1 << 15) != 0 {
                return 15;
            }
            let extras = input & ((1u64 << 62) - 1) & !binding::NORMAL.iter().fold(0, |m, &b| m | 1 << b);
            let real = input & ((1u64 << 62) - 1);
            if extras != 0 {
                extras.trailing_zeros() as u8
            } else if real != 0 {
                real.trailing_zeros() as u8
            } else {
                15
            }
        })
    }

    pub fn enabled_shortcuts(&self) -> impl Iterator<Item = (&u64, &Chord)> + Clone {
        self.shortcuts.iter().filter(|(mask, _)| self.button_mode(self.shortcut_anchor(**mask)) == Mode::Combinations)
    }

    pub fn sanitise_button_modes(&mut self) {
        self.button_modes.retain(|&b, _| b < 62);
        let explicit_modes: std::collections::BTreeSet<u8> = self.button_modes.keys().copied().collect();
        self.button_presets.retain(|&b, _| b < 62);
        self.shortcut_anchors.retain(|mask, anchor| self.shortcuts.contains_key(mask) && *anchor < 62);
        for preset in self.button_presets.values_mut() {
            preset.sanitise();
        }
        for (&button, action) in &self.bindings {
            if explicit_modes.contains(&button) {
                self.button_presets.entry(button).or_default().remember(action);
            }
        }
        for input in self.shortcuts.keys().copied().collect::<Vec<_>>() {
            let anchor = self.shortcut_anchor(input);
            self.shortcut_anchors.insert(input, anchor);
            if !explicit_modes.contains(&anchor) {
                // Earlier files had a flat combination list and no explicit active modes.
                self.select_button_mode(anchor, Mode::Combinations);
            }
        }
        for (&button, &mode) in &self.button_modes {
            self.bindings.remove(&button);
            if let Some(action) = self.button_presets.get(&button).and_then(|p| p.action(mode)) {
                self.bindings.insert(button, action);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{Edit, Profiles};
    use crate::shortcuts::{self, State};
    const CAPTURE: u8 = 15;
    const INPUT: u64 = (1 << CAPTURE) | 1;
    const F10: Chord = Chord::new(0, 121);
    const HOTKEY: Chord = Chord::new(5, 122);

    #[test]
    fn capture_f10_switches_to_native_combinations_and_back_without_reset() {
        let mut p = Profiles::default();
        p.apply(Edit::Bind { button: CAPTURE, action: Some(Action::Keys(F10)) });
        p.apply(Edit::ButtonMode { button: CAPTURE, mode: Mode::Combinations });
        p.apply(Edit::Shortcut { name: None, anchor: CAPTURE, input: INPUT, keys: Some(HOTKEY) });
        let mut state = State::default();
        let out = state.update(p.active().enabled_shortcuts(), &p.active().bindings, 0, INPUT);
        assert_eq!(out.fire, Some(HOTKEY));
        assert_eq!(out.after, 0);
        assert!(out.taps.is_empty());
        assert!(!p.active().bindings.contains_key(&CAPTURE), "F10 is remembered, not active");
        assert!(state.update(p.active().enabled_shortcuts(), &p.active().bindings, INPUT, 0).taps.is_empty());
        p.apply(Edit::ButtonMode { button: CAPTURE, mode: Mode::Key });
        assert_eq!(p.active().bindings.get(&CAPTURE), Some(&Action::Keys(F10)));
        assert_eq!(p.active().enabled_shortcuts().count(), 0);
        assert_eq!(p.active().shortcuts.get(&INPUT), Some(&HOTKEY));
        p.apply(Edit::ButtonMode { button: CAPTURE, mode: Mode::Combinations });
        let restored: Profiles = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        let restored = restored.sanitised();
        assert_eq!(restored.active().enabled_shortcuts().count(), 1);
        assert!(!restored.active().bindings.contains_key(&CAPTURE));
    }

    #[test]
    fn selecting_key_survives_reload_and_leaves_combinations_disabled() {
        let mut p = Profiles::default();
        p.apply(Edit::Bind { button: CAPTURE, action: Some(Action::Keys(F10)) });
        p.apply(Edit::Shortcut { name: None, anchor: CAPTURE, input: INPUT, keys: Some(HOTKEY) });
        p.apply(Edit::ButtonMode { button: CAPTURE, mode: Mode::Key });
        let p: Profiles = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        let p = p.sanitised();
        assert_eq!(p.active().button_mode(CAPTURE), Mode::Key);
        assert_eq!(p.active().enabled_shortcuts().count(), 0);
        assert_eq!(p.active().bindings[&CAPTURE], Action::Keys(F10));
    }

    #[test]
    fn legacy_conflicting_f10_is_kept_in_the_key_preset_when_migrated() {
        let p: Profiles = serde_json::from_str(
            r#"{"list":[{"bindings":{"15":{"Keys":{"mods":0,"key":121}}},"shortcuts":{"32769":{"mods":5,"key":122}}}]}"#,
        )
        .unwrap();
        let mut p = p.sanitised();
        assert_eq!(p.active().button_mode(CAPTURE), Mode::Combinations);
        assert!(!p.active().bindings.contains_key(&CAPTURE));
        p.apply(Edit::ButtonMode { button: CAPTURE, mode: Mode::Key });
        assert_eq!(p.active().bindings[&CAPTURE], Action::Keys(F10));
    }

    #[test]
    fn macro_xbox_and_nothing_keep_independent_presets() {
        let mut p = Profile::default();
        p.assign_button(CAPTURE, Some(Action::Keys(F10)));
        let macro_action = Action::Macro(vec![Step::Keys(HOTKEY)]);
        p.assign_button(CAPTURE, Some(macro_action.clone()));
        p.assign_button(CAPTURE, Some(Action::Xbox(crate::binding::XboxButton::A)));
        for mode in [Mode::Nothing, Mode::Combinations] {
            p.select_button_mode(CAPTURE, mode);
            assert!(!p.bindings.contains_key(&CAPTURE));
        }
        p.select_button_mode(CAPTURE, Mode::Macro);
        assert_eq!(p.bindings[&CAPTURE], macro_action);
        p.select_button_mode(CAPTURE, Mode::Key);
        assert_eq!(p.bindings[&CAPTURE], Action::Keys(F10));
        p.select_button_mode(CAPTURE, Mode::Xbox);
        assert_eq!(p.bindings[&CAPTURE], Action::Xbox(crate::binding::XboxButton::A));
        assert_eq!(shortcuts::input(&crate::mapping::PadState { buttons: 1 << CAPTURE, ..Default::default() }), 1 << CAPTURE);
    }
}
