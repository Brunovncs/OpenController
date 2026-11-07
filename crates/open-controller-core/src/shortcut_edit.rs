//! Atomic shortcut edits. Unassigned actions belong to a profile, never to another editor.
use crate::{
    binding::Chord,
    button_mode::Mode,
    profile::Profile,
    shortcuts::{self, ShortcutAction},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    #[serde(default)]
    pub hold_until_release: bool,
    #[serde(default = "shortcuts::default_hold_ms")]
    pub hold_ms: u16,
    pub profile: usize,
    pub original: Option<u64>,
    pub unassigned: Option<u64>,
    pub anchor: u8,
    pub input: Option<u64>,
    pub keys: Chord,
    pub name: String,
    pub replace: bool,
}

impl Profile {
    pub fn shortcut_hold_ms(&self, input: u64) -> u16 {
        shortcuts::hold_ms(self.shortcut_durations.get(&input).copied().unwrap_or(shortcuts::DEFAULT_HOLD_MS))
    }

    pub fn stash_shortcut(&mut self, action: ShortcutAction) -> bool {
        if self.unassigned_shortcuts.values().any(|a| a == &action) {
            return true;
        }
        let Some(id) = (0..shortcuts::MAX_UNASSIGNED as u64).find(|id| !self.unassigned_shortcuts.contains_key(id)) else {
            return false;
        };
        self.unassigned_shortcuts.insert(id, action);
        true
    }

    pub fn change_shortcut(&mut self, change: Change) -> bool {
        let mut edited = self.clone();
        if !edited.try_change_shortcut(change) {
            return false;
        }
        *self = edited;
        true
    }

    fn try_change_shortcut(&mut self, c: Change) -> bool {
        if c.anchor >= 62
            || c.keys.is_empty()
            || c.keys.mods & !15 != 0
            || c.keys.key > 255
            || c.input.is_some_and(|input| !shortcuts::valid(input, c.keys) || input & (1 << c.anchor) == 0)
            || c.original.is_some_and(|mask| !self.shortcuts.contains_key(&mask))
            || c.unassigned.is_some_and(|id| !self.unassigned_shortcuts.contains_key(&id))
            || (c.original.is_some() && c.unassigned.is_some())
        {
            return false;
        }
        if let Some(input) = c.input {
            if Some(input) != c.original && self.shortcuts.contains_key(&input) {
                if !c.replace {
                    return false;
                }
                let displaced = ShortcutAction {
                    hold_ms: self.shortcut_hold_ms(input),
                    hold_until_release: self.shortcut_held.contains(&input),
                    name: self.shortcut_names.get(&input).cloned().unwrap_or_default(),
                    keys: self.shortcuts[&input],
                    anchor: self.shortcut_anchor(input),
                };
                if !self.stash_shortcut(displaced) {
                    return false;
                }
            }
            if !self.shortcuts.contains_key(&input) && c.original.is_none() && self.shortcuts.len() >= shortcuts::MAX_SHORTCUTS {
                return false;
            }
        }
        if let Some(old) = c.original {
            self.shortcuts.remove(&old);
            self.shortcut_durations.remove(&old);
            self.shortcut_held.remove(&old);
            self.shortcut_names.remove(&old);
            self.shortcut_anchors.remove(&old);
        }
        if let Some(id) = c.unassigned {
            self.unassigned_shortcuts.remove(&id);
        }
        let name = shortcuts::clean_name(&c.name);
        match c.input {
            Some(input) => {
                self.shortcuts.insert(input, c.keys);
                self.shortcut_held.remove(&input);
                if c.hold_until_release {
                    self.shortcut_held.insert(input);
                }
                let ms = shortcuts::hold_ms(c.hold_ms);
                self.shortcut_durations.remove(&input);
                if ms != shortcuts::DEFAULT_HOLD_MS {
                    self.shortcut_durations.insert(input, ms);
                }
                self.shortcut_anchors.insert(input, c.anchor);
                self.shortcut_names.remove(&input);
                if !name.is_empty() {
                    self.shortcut_names.insert(input, name);
                }
            }
            None => {
                if !self.stash_shortcut(ShortcutAction {
                    name,
                    keys: c.keys,
                    anchor: c.anchor,
                    hold_ms: shortcuts::hold_ms(c.hold_ms),
                    hold_until_release: c.hold_until_release,
                }) {
                    return false;
                }
            }
        }
        self.select_button_mode(c.anchor, Mode::Combinations);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{Edit, Profiles};
    fn change() -> Change {
        Change {
            hold_until_release: false,
            hold_ms: 250,
            profile: 0,
            original: None,
            unassigned: None,
            anchor: 15,
            input: Some(32769),
            keys: Chord::new(3, 5),
            name: "OBS pause".into(),
            replace: false,
        }
    }
    #[test]
    fn replacing_moves_the_displaced_action_to_unassigned_and_is_atomic() {
        let mut p = Profile::default();
        assert!(p.change_shortcut(change()));
        let old = p.clone();
        let replacement = Change { name: "Recording".into(), keys: Chord::new(3, 6), ..change() };
        assert!(!p.change_shortcut(replacement.clone()));
        assert_eq!(p, old);
        assert!(p.change_shortcut(Change { replace: true, ..replacement }));
        assert_eq!(p.shortcuts[&32769].key, 6);
        assert_eq!(p.unassigned_shortcuts[&0].name, "OBS pause");
    }
    #[test]
    fn unassign_reassign_and_roundtrip_keep_name_and_output() {
        let mut p = Profile::default();
        p.change_shortcut(change());
        assert!(p.change_shortcut(Change { original: Some(32769), input: None, ..change() }));
        assert!(p.shortcuts.is_empty());
        assert_eq!(p.unassigned_shortcuts[&0].keys.key, 5);
        assert_eq!(p.unassigned_shortcuts[&0].hold_ms, 250);
        let text = serde_json::to_string(&p).unwrap();
        let mut p: Profile = serde_json::from_str(&text).unwrap();
        assert!(p.change_shortcut(Change { unassigned: Some(0), input: Some(32768), ..change() }));
        assert!(p.unassigned_shortcuts.is_empty());
        assert_eq!(p.shortcut_names[&32768], "OBS pause");
        assert_eq!(p.shortcut_hold_ms(32768), 250);
    }
    #[test]
    fn new_profiles_are_empty_and_stale_editor_changes_cannot_cross_profiles() {
        let mut p = Profiles::default();
        p.apply(Edit::ChangeShortcut(change()));
        let first = p.list[0].clone();
        p.apply(Edit::Add("New".into()));
        assert!(p.active().shortcuts.is_empty());
        p.apply(Edit::ChangeShortcut(change()));
        assert!(p.active().shortcuts.is_empty());
        assert_eq!(p.list[0], first);
        p.apply(Edit::ChangeShortcut(Change { profile: 1, ..change() }));
        assert_eq!(p.active().shortcuts.len(), 1);
        assert_eq!(p.list[0], first);
    }
    #[test]
    fn held_mode_and_remembered_duration_survive_unassignment_and_reassignment() {
        let mut p = Profile::default();
        p.change_shortcut(Change { hold_until_release: true, hold_ms: 60_000, ..change() });
        assert!(p.shortcut_held.contains(&32769));
        assert_eq!(p.shortcut_hold_ms(32769), 60_000);
        p.change_shortcut(Change { hold_until_release: true, hold_ms: 60_000, original: Some(32769), input: None, ..change() });
        assert!(p.unassigned_shortcuts[&0].hold_until_release);
        let restored: Profile = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        p = restored;
        p.change_shortcut(Change {
            hold_until_release: true,
            hold_ms: p.unassigned_shortcuts[&0].hold_ms,
            unassigned: Some(0),
            ..change()
        });
        assert!(p.shortcut_held.contains(&32769));
        p.change_shortcut(Change { original: Some(32769), hold_until_release: false, hold_ms: 60_000, ..change() });
        assert!(!p.shortcut_held.contains(&32769));
        assert_eq!(p.shortcut_hold_ms(32769), 60_000);
    }
}
