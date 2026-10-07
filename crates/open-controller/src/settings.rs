//! Preferences, kept in `settings.json` in the app data folder.

use open_controller_core::i18n::Lang;
use open_controller_core::platform::VirtualDriver;
use open_controller_core::profile::Profiles;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::io::Write;
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
    /// The driver that makes the virtual controllers on Windows: ViGEmBus, or the experimental
    /// VIIPER. A value this version does not know reads as ViGEmBus; a version without the field
    /// ignores it.
    pub virtual_driver: VirtualDriver,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            hide_originals: true,
            check_updates: true,
            language: Lang::En,
            controllers: BTreeMap::new(),
            virtual_driver: VirtualDriver::ViGEmBus,
        }
    }
}

impl Settings {
    pub fn load(dir: &Path) -> Settings {
        let path = dir.join(FILE_NAME);
        let Ok(bytes) = std::fs::read(&path) else { return Settings::default() };
        let mut s = serde_json::from_slice::<Settings>(&bytes).unwrap_or_else(|_| {
            // A value this version does not know (written by a newer one, or edited by hand)
            // must not take every assignment with it on the next save: the file is kept as it
            // was, and whatever can be read of it is.
            let _ = std::fs::copy(&path, dir.join(format!("{FILE_NAME}.bad")));
            Settings::salvage(&bytes)
        });
        s.controllers = std::mem::take(&mut s.controllers).into_iter().map(|(k, v)| (k, v.sanitised())).collect();
        s
    }

    /// Each field on its own, and each controller's profiles on their own.
    fn salvage(bytes: &[u8]) -> Settings {
        let mut s = Settings::default();
        let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(bytes) else { return s };
        fn field<T: DeserializeOwned>(map: &Map<String, Value>, key: &str) -> Option<T> {
            map.get(key).and_then(|v| T::deserialize(v).ok())
        }
        if let Some(v) = field(&map, "hide_originals") {
            s.hide_originals = v;
        }
        if let Some(v) = field(&map, "check_updates") {
            s.check_updates = v;
        }
        if let Some(v) = field(&map, "language") {
            s.language = v;
        }
        if let Some(v) = field(&map, "virtual_driver") {
            s.virtual_driver = v;
        }
        if let Some(Value::Object(c)) = map.get("controllers") {
            s.controllers = c.iter().filter_map(|(k, v)| Some((k.clone(), Profiles::deserialize(v).ok()?))).collect();
        }
        s
    }

    /// Written to a temporary file first and flushed to the disk before it replaces the old
    /// one, so neither a crash nor a power cut mid-write can lose the assignments.
    pub fn save(&self, dir: &Path) {
        let Ok(text) = serde_json::to_vec_pretty(self) else { return };
        let tmp = dir.join(format!("{FILE_NAME}.tmp"));
        let written = std::fs::File::create(&tmp).and_then(|mut f| {
            f.write_all(&text)?;
            f.sync_all()
        });
        match written.and_then(|()| std::fs::rename(&tmp, dir.join(FILE_NAME))) {
            Ok(()) => {}
            Err(e) => eprintln!("open-controller: could not save the settings: {e}"),
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
    fn a_file_this_version_cannot_read_keeps_what_it_can() {
        let dir = std::env::temp_dir().join(format!("oc-settings-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let text = r#"{"hide_originals": false, "language": "Klingon",
            "controllers": {"serial:a": {"list": [{"name": "Racing"}], "active": 0}, "serial:b": {"list": "broken"}}}"#;
        std::fs::write(dir.join(FILE_NAME), text).unwrap();
        let s = Settings::load(&dir);
        assert!(!s.hide_originals);
        assert_eq!(s.language, Lang::En);
        assert!(s.controllers.contains_key("serial:a"), "{:?}", s.controllers.keys());
        assert!(!s.controllers.contains_key("serial:b"));
        assert_eq!(std::fs::read_to_string(dir.join("settings.json.bad")).unwrap(), text, "the original is kept");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_from_0_8_0_loads_the_same() {
        use open_controller_core::profile::{Gyro, GyroMode};
        let dir = std::env::temp_dir().join(format!("oc-settings-080-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let text = r#"{"hide_originals": true, "check_updates": false, "controllers": {
            "serial:a": {"list": [{"name": "", "gyro": {"mode": {"Holding": 9}, "sensitivity": 150, "invert_y": true}}], "active": 0},
            "serial:b": {"list": [{"name": "Racing", "gyro": {"mode": "Off", "sensitivity": 100, "invert_y": false}}], "active": 0}}}"#;
        std::fs::write(dir.join(FILE_NAME), text).unwrap();
        let s = Settings::load(&dir);
        assert!(!dir.join("settings.json.bad").exists(), "read as it is, nothing salvaged");
        assert_eq!(
            s.controllers["serial:a"].active().gyro,
            Gyro { mode: GyroMode::Holding(9), sensitivity: 150, invert_y: true, ..Gyro::default() }
        );
        assert_eq!(s.controllers["serial:b"].active().gyro, Gyro::default());
        s.save(&dir);
        assert_eq!(Settings::load(&dir), s);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_virtual_driver_never_costs_the_rest() {
        let dir = std::env::temp_dir().join(format!("oc-settings-driver-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // From a newer version that knows a driver this one does not.
        let text = r#"{"hide_originals": false, "virtual_driver": "SomethingNewer",
            "controllers": {"serial:a": {"list": [{"name": "Racing"}], "active": 0}}}"#;
        std::fs::write(dir.join(FILE_NAME), text).unwrap();
        let s = Settings::load(&dir);
        assert_eq!(s.virtual_driver, VirtualDriver::ViGEmBus);
        assert!(!s.hide_originals);
        assert!(s.controllers.contains_key("serial:a"));
        assert!(!dir.join("settings.json.bad").exists(), "nothing to salvage");
        // Chosen here, kept across a save.
        let chosen = Settings { virtual_driver: VirtualDriver::Viiper, ..s };
        chosen.save(&dir);
        assert_eq!(Settings::load(&dir), chosen);
        // 0.8.0 reads the same file: the field it does not know is skipped, the rest kept.
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct Before {
            hide_originals: bool,
            controllers: BTreeMap<String, Profiles>,
        }
        let old: Before = serde_json::from_slice(&std::fs::read(dir.join(FILE_NAME)).unwrap()).unwrap();
        assert!(!old.hide_originals);
        assert!(old.controllers.contains_key("serial:a"));
        let _ = std::fs::remove_dir_all(&dir);
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

    #[test]
    fn a_native_controller_survives_a_save() {
        use open_controller_core::profile::Edit;
        let dir = std::env::temp_dir().join(format!("oc-settings-native-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = Settings::default();
        s.controllers.entry("model:054c:0ce6".into()).or_default().apply(Edit::Native(true));
        s.save(&dir);
        let back = Settings::load(&dir);
        assert!(back.controllers["model:054c:0ce6"].native);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
