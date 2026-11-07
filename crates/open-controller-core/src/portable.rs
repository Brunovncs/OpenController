//! Portable controller profiles and a controller-independent library of named output actions.
use crate::device::Form;
use crate::extras::Family;
use crate::{
    PadView,
    binding::{Action, Chord},
    keyboard,
    profile::Profiles,
    shortcuts,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    io::{Read, Write},
    path::Path,
};

pub const MAX_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_ACTIONS: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerIdentity {
    pub name: String,
    pub vendor: u16,
    pub product: u16,
    pub family: Family,
    pub form: Form,
    pub guid: String,
    pub store: Option<String>,
}

impl From<&PadView> for ControllerIdentity {
    fn from(p: &PadView) -> Self {
        Self {
            name: p.name.clone(),
            vendor: p.vendor,
            product: p.product,
            family: p.family,
            form: p.form,
            guid: p.guid.clone(),
            store: p.store.clone(),
        }
    }
}

impl ControllerIdentity {
    pub fn compatible(&self, target: &Self) -> bool {
        let same_model = self.vendor == target.vendor
            && self.product == target.product
            && self.family == target.family
            && (self.vendor != 0 || (!self.guid.is_empty() && self.guid == target.guid));
        self.form == target.form && same_model
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerFile {
    pub format: String,
    pub version: u32,
    pub controller: ControllerIdentity,
    pub profiles: Profiles,
    pub actions: ActionLibrary,
}

impl ControllerFile {
    pub fn new(pad: &PadView, library: ActionLibrary) -> Result<Self, String> {
        Ok(Self {
            format: "opencontroller.configuration".into(),
            version: 1,
            controller: pad.into(),
            profiles: pad.profiles.clone().sanitised(),
            actions: library.with_profiles(&pad.profiles)?,
        })
    }

    /// A mismatch can always import global actions, but must never apply controller inputs.
    pub fn profiles_for(&self, target: &ControllerIdentity, full: bool) -> Result<Option<Profiles>, String> {
        if !full {
            return Ok(None);
        }
        if !self.controller.compatible(target) {
            return Err("The controller does not match this configuration.".into());
        }
        Ok(Some(self.profiles.clone().sanitised()))
    }

    /// Global import creates unassigned rows only in the selected profile.
    pub fn global_profiles(&self, target: &Profiles, anchor: u8) -> Result<Profiles, String> {
        let mut profiles = target.clone();
        let p = profiles.list.get_mut(target.active).ok_or("No selected profile.")?;
        for a in &self.actions.actions {
            let bound = p.shortcuts.iter().any(|(input, keys)| {
                *keys == a.keys
                    && p.shortcut_hold_ms(*input) == a.hold_ms
                    && p.shortcut_held.contains(input) == a.hold_until_release
                    && p.shortcut_names.get(input).cloned().unwrap_or_else(|| keyboard::chord_names(*keys).join(" + ")) == a.name
            });
            let unassigned = p
                .unassigned_shortcuts
                .values()
                .any(|s| s.keys == a.keys && s.name == a.name && s.hold_ms == a.hold_ms && s.hold_until_release == a.hold_until_release);
            if !bound
                && !unassigned
                && !p.stash_shortcut(shortcuts::ShortcutAction {
                    name: a.name.clone(),
                    keys: a.keys,
                    anchor,
                    hold_ms: a.hold_ms,
                    hold_until_release: a.hold_until_release,
                })
            {
                return Err("The selected profile has too many unassigned actions (maximum 256).".into());
            }
        }
        p.select_button_mode(anchor, crate::button_mode::Mode::Combinations);
        Ok(profiles)
    }

    pub fn validate(mut self) -> Result<Self, String> {
        if self.format != "opencontroller.configuration" || self.version != 1 {
            return Err("Unsupported configuration format or version.".into());
        }
        if self.profiles.list.is_empty() {
            return Err("The configuration contains no profiles.".into());
        }
        self.profiles = self.profiles.sanitised();
        self.actions = self.actions.validate()?;
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedAction {
    #[serde(default)]
    pub hold_until_release: bool,
    #[serde(default = "shortcuts::default_hold_ms")]
    pub hold_ms: u16,
    pub name: String,
    pub keys: Chord,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionLibrary {
    pub format: String,
    pub version: u32,
    pub actions: Vec<NamedAction>,
}

impl Default for ActionLibrary {
    fn default() -> Self {
        Self { format: "opencontroller.actions".into(), version: 1, actions: Vec::new() }
    }
}

impl ActionLibrary {
    pub fn validate(mut self) -> Result<Self, String> {
        if self.format != "opencontroller.actions" || self.version != 1 {
            return Err("Unsupported actions library format or version.".into());
        }
        if self.actions.len() > MAX_ACTIONS {
            return Err("The library contains more than 256 actions.".into());
        }
        let mut unique = Vec::new();
        for mut a in self.actions {
            if a.keys.is_empty() || a.keys.mods & !15 != 0 || a.keys.key > 255 {
                return Err("An action contains an invalid keyboard or mouse shortcut.".into());
            }
            a.name = shortcuts::clean_name(&a.name);
            a.hold_ms = shortcuts::hold_ms(a.hold_ms);
            if a.name.is_empty() {
                a.name = keyboard::chord_names(a.keys).join(" + ");
            }
            if !unique.contains(&a) {
                unique.push(a);
            }
        }
        unique.sort_by_key(|a| (a.name.to_lowercase(), a.keys.mods, a.keys.keys().collect::<Vec<_>>(), a.hold_ms, a.hold_until_release));
        self.actions = unique;
        Ok(self)
    }

    pub fn with_profiles(mut self, profiles: &Profiles) -> Result<Self, String> {
        for p in &profiles.list {
            for a in p.unassigned_shortcuts.values() {
                let a = NamedAction { name: a.name.clone(), keys: a.keys, hold_ms: a.hold_ms, hold_until_release: a.hold_until_release };
                if !self.actions.contains(&a) {
                    self.actions.push(a);
                }
            }
            for (&input, &keys) in &p.shortcuts {
                let a = NamedAction {
                    hold_ms: p.shortcut_hold_ms(input),
                    hold_until_release: p.shortcut_held.contains(&input),
                    name: p.shortcut_names.get(&input).cloned().unwrap_or_else(|| keyboard::chord_names(keys).join(" + ")),
                    keys,
                };
                if !self.actions.contains(&a) {
                    self.actions.push(a);
                }
            }
            for action in p.bindings.values() {
                if let Action::Keys(keys) = action {
                    let a = NamedAction {
                        name: keyboard::chord_names(*keys).join(" + "),
                        keys: *keys,
                        hold_ms: shortcuts::DEFAULT_HOLD_MS,
                        hold_until_release: false,
                    };
                    if !self.actions.iter().any(|a| a.keys == *keys) {
                        self.actions.push(a);
                    }
                }
            }
        }
        self.validate()
    }

    pub fn merge(mut self, other: Self) -> Result<Self, String> {
        for a in other.validate()?.actions {
            if !self.actions.contains(&a) {
                self.actions.push(a);
            }
        }
        self.validate()
    }
}

pub fn read<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path).and_then(|f| f.take(MAX_BYTES + 1).read_to_end(&mut bytes)).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Configuration file is too large (maximum 4 MB).".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("Invalid configuration: {e}"))
}

pub fn write<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    let tmp = path.with_extension(format!("oc-tmp-{}-{stamp}", std::process::id()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{binding::modifier, profile::Edit};
    fn identity() -> ControllerIdentity {
        ControllerIdentity {
            name: "Switch Pro".into(),
            vendor: 0x57e,
            product: 0x2009,
            family: Family::SwitchPro,
            form: Form::Gamepad,
            guid: "switch".into(),
            store: Some("serial:original".into()),
        }
    }
    #[test]
    fn global_actions_preserve_every_key_in_a_multi_key_output() {
        let keys = Chord::from_keys(modifier::CTRL, [0x56, 0x26]);
        let library = ActionLibrary {
            actions: vec![NamedAction { name: "AudioSync volume".into(), keys, hold_ms: 100, hold_until_release: true }],
            ..ActionLibrary::default()
        };
        let json = serde_json::to_string(&library).unwrap();
        let loaded: ActionLibrary = serde_json::from_str(&json).unwrap();
        let loaded = loaded.validate().unwrap();
        assert_eq!(loaded.actions[0].keys, keys);
        assert!(loaded.actions[0].hold_until_release);
    }

    #[test]
    fn controller_file_is_model_bound_but_works_with_a_new_store_on_another_pc() {
        let source = identity();
        let mut target = source.clone();
        target.store = Some("serial:other-pc".into());
        let file = ControllerFile {
            format: "opencontroller.configuration".into(),
            version: 1,
            controller: source,
            profiles: Profiles::default(),
            actions: ActionLibrary::default(),
        };
        assert!(file.controller.compatible(&target));
        assert!(file.clone().validate().is_ok());
        assert!(file.profiles_for(&target, true).unwrap().is_some());
        target.vendor = 0x54c;
        target.family = Family::DualSense;
        assert!(!file.controller.compatible(&target));
        assert!(file.profiles_for(&target, true).is_err());
        assert!(file.profiles_for(&target, false).unwrap().is_none());
        assert!(file.validate().is_ok(), "global actions remain usable for another controller");
    }
    #[test]
    fn global_actions_roundtrip_without_controller_inputs_and_preserve_mouse_modifiers() {
        let mut p = Profiles::default();
        p.apply(Edit::Shortcut {
            anchor: 15,
            input: (1 << 15) | 1,
            name: Some("OBS recording".into()),
            keys: Some(Chord::new(modifier::CTRL | modifier::SHIFT, 5)),
        });
        let library = ActionLibrary::default().with_profiles(&p).unwrap();
        let text = serde_json::to_string(&library).unwrap();
        assert!(!text.contains("anchor") && !text.contains("serial") && !text.contains("32769"));
        let loaded: ActionLibrary = serde_json::from_str(&text).unwrap();
        assert_eq!(loaded.validate().unwrap(), library);
        assert_eq!(library.actions[0].keys, Chord::new(3, 5));
        assert_eq!(library.clone().merge(library.clone()).unwrap(), library);
    }
    #[test]
    fn wrong_file_types_and_invalid_actions_are_rejected() {
        let library = ActionLibrary { version: 99, ..Default::default() };
        assert!(library.validate().is_err());
        let library = ActionLibrary {
            actions: vec![NamedAction { name: "bad".into(), keys: Chord::new(32, 5), hold_ms: 100, hold_until_release: false }],
            ..Default::default()
        };
        assert!(library.validate().is_err());
    }
    #[test]
    fn a_single_file_roundtrips_and_overwriting_leaves_no_side_files() {
        let dir = std::env::temp_dir().join(format!("oc-portable-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("configuration.json");
        let mut profiles = Profiles::default();
        profiles.apply(Edit::Shortcut { anchor: 15, input: (1 << 15) | 1, name: Some("OBS".into()), keys: Some(Chord::new(3, 5)) });
        let file = ControllerFile {
            format: "opencontroller.configuration".into(),
            version: 1,
            controller: identity(),
            actions: ActionLibrary::default().with_profiles(&profiles).unwrap(),
            profiles,
        };
        write(&path, &file).unwrap();
        write(&path, &file).unwrap();
        assert_eq!(read::<ControllerFile>(&path).unwrap().validate().unwrap(), file);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::write(&path, vec![b' '; (MAX_BYTES + 1) as usize]).unwrap();
        assert!(read::<ControllerFile>(&path).unwrap_err().contains("too large"));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn global_import_creates_unassigned_rows_only_in_the_selected_profile_without_duplicates() {
        let mut source = Profiles::default();
        source.apply(Edit::Shortcut { anchor: 15, input: 32769, name: Some("OBS".into()), keys: Some(Chord::new(3, 5)) });
        source.list[0].shortcut_durations.insert(32769, 250);
        source.list[0].shortcut_held.insert(32769);
        let file = ControllerFile {
            format: "opencontroller.configuration".into(),
            version: 1,
            controller: identity(),
            actions: ActionLibrary::default().with_profiles(&source).unwrap(),
            profiles: source.clone(),
        };
        let mut target = source;
        target.apply(Edit::Add("Other".into()));
        let before = target.list[0].clone();
        let imported = file.global_profiles(&target, 15).unwrap();
        assert_eq!(imported.list[0], before);
        assert!(imported.active().shortcuts.is_empty());
        assert_eq!(imported.active().unassigned_shortcuts.len(), 1);
        assert_eq!(imported.active().unassigned_shortcuts[&0].hold_ms, 250);
        assert!(imported.active().unassigned_shortcuts[&0].hold_until_release);
        assert_eq!(file.global_profiles(&imported, 15).unwrap(), imported);
        let exported = ActionLibrary::default().with_profiles(&imported).unwrap();
        assert_eq!(exported.actions.len(), 1);
    }
}
