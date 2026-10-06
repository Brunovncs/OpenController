//! Preferences, kept in `settings.json` in the app data folder.

use open_controller_core::i18n::Lang;
use open_controller_core::profile::Profiles;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const FILE_NAME: &str = "settings.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Hide the original controllers from games while OpenController runs (needs HidHide).
    pub hide_originals: bool,
    /// Ask GitHub for a newer version when the window opens.
    pub check_updates: bool,
    /// The interface's language; English until another is picked.
    pub language: Lang,
    /// Each controller's profiles, by where its settings are kept.
    pub controllers: BTreeMap<String, Profiles>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { hide_originals: true, check_updates: true, language: Lang::En, controllers: BTreeMap::new() }
    }
}

impl Settings {
    pub fn load(dir: &Path) -> Settings {
        let mut s: Settings = std::fs::read(dir.join(FILE_NAME)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        s.controllers = std::mem::take(&mut s.controllers).into_iter().map(|(k, v)| (k, v.sanitised())).collect();
        s
    }

    /// Written to a temporary file first, so a crash mid-write cannot lose the assignments.
    pub fn save(&self, dir: &Path) {
        let Ok(text) = serde_json::to_vec_pretty(self) else { return };
        let tmp = dir.join(format!("{FILE_NAME}.tmp"));
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, dir.join(FILE_NAME));
        }
    }
}

pub fn default_data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("io.github.brunovncs.open-controller")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_unknown_fields_are_tolerated() {
        let s: Settings = serde_json::from_str(r#"{"from_a_later_version": 1}"#).unwrap();
        assert_eq!(s, Settings::default());
        let s: Settings = serde_json::from_str(r#"{"hide_originals": false}"#).unwrap();
        assert!(!s.hide_originals);
    }

    #[test]
    fn profiles_survive_a_save() {
        use open_controller_core::binding::{Action, Chord, Step, XboxButton};
        use open_controller_core::profile::{Edit, Light};
        let dir = std::env::temp_dir().join(format!("oc-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = Settings::default();
        let p = s.controllers.entry("serial:e417d8bc366c".into()).or_default();
        p.apply(Edit::Bind { button: 17, action: Some(Action::Xbox(XboxButton::A)) });
        p.apply(Edit::Add("Racing".into()));
        p.apply(Edit::Bind { button: 19, action: Some(Action::Macro(vec![Step::Keys(Chord { mods: 1, key: 0x43 }), Step::Wait(40)])) });
        p.apply(Edit::Light(Light::Color([255, 80, 0])));
        s.save(&dir);
        assert_eq!(Settings::load(&dir), s);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
