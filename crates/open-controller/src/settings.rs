//! Preferences, kept in `settings.json` in the app data folder.

use open_controller_core::i18n::Lang;
use open_controller_core::platform::VirtualDriver;
use open_controller_core::profile::{Profiles, readable_fields};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

const FILE_NAME: &str = "settings.json";

/// Set when `settings.json` could be neither read nor moved aside: it still holds the user's
/// assignments, so no save may replace it.
static UNREAD: AtomicBool = AtomicBool::new(false);

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
    /// Each Joy-Con a controller of its own, held sideways, rather than a left and a right one
    /// joined into one.
    pub split_joycons: bool,
    /// Gamepad mappings made in the window for joysticks SDL has no layout for, by GUID.
    pub mappings: BTreeMap<String, String>,
    /// Fields a newer version wrote, kept as they were so saving here does not take them away.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            hide_originals: true,
            check_updates: true,
            language: Lang::En,
            controllers: BTreeMap::new(),
            virtual_driver: VirtualDriver::ViGEmBus,
            split_joycons: false,
            mappings: BTreeMap::new(),
            extra: Map::new(),
        }
    }
}

impl Settings {
    pub fn load(dir: &Path) -> Settings {
        let path = dir.join(FILE_NAME);
        let Some(bytes) = read(&path) else { return Settings::default() };
        let mut s = serde_json::from_slice::<Settings>(&bytes).unwrap_or_else(|_| {
            // A value this version does not know (written by a newer one, or edited by hand)
            // must not take every assignment with it on the next save: the file is kept as it
            // was, and whatever can be read of it is.
            keep_bad(dir, &bytes);
            Settings::salvage(&bytes)
        });
        s.controllers = std::mem::take(&mut s.controllers).into_iter().map(|(k, v)| (k, v.sanitised())).collect();
        s
    }

    /// Each field on its own, and each controller's profiles on their own.
    fn salvage(bytes: &[u8]) -> Settings {
        let Ok(Value::Object(mut map)) = serde_json::from_slice::<Value>(bytes) else { return Settings::default() };
        let controllers = map.remove("controllers");
        let mut s: Settings = readable_fields(&map);
        if let Some(Value::Object(c)) = controllers {
            s.controllers = c.iter().filter_map(|(k, v)| Some((k.clone(), Profiles::salvage(v)?))).collect();
        }
        s
    }

    /// Written to a temporary file first and flushed to the disk before it replaces the old
    /// one, so neither a crash nor a power cut mid-write can lose the assignments.
    pub fn save(&self, dir: &Path) {
        let path = dir.join(FILE_NAME);
        if UNREAD.load(Ordering::Relaxed) {
            if let Err(e) = std::fs::rename(&path, path.with_extension("json.unread")) {
                eprintln!("open-controller: not saving the settings over a file that could not be read: {e}");
                return;
            }
            UNREAD.store(false, Ordering::Relaxed);
        }
        let Ok(text) = serde_json::to_vec_pretty(self) else { return };
        let tmp = dir.join(format!("{FILE_NAME}.tmp"));
        let written = std::fs::File::create(&tmp).and_then(|mut f| {
            f.write_all(&text)?;
            f.sync_all()
        });
        match written.and_then(|()| std::fs::rename(&tmp, &path)) {
            // The rename itself lives in the folder: on Linux and macOS it reaches the disk only
            // once the folder is flushed too.
            #[cfg(unix)]
            Ok(()) => {
                if let Err(e) = std::fs::File::open(dir).and_then(|d| d.sync_all()) {
                    eprintln!("open-controller: could not flush the settings folder: {e}");
                }
            }
            #[cfg(not(unix))]
            Ok(()) => {}
            Err(e) => eprintln!("open-controller: could not save the settings: {e}"),
        }
    }
}

/// How many copies of files that did not read as a whole are kept.
const MAX_BAD: usize = 3;

/// A copy of a file that did not read as a whole, before a save replaces it with what could be
/// read of it. Copies are never overwritten: the first is the one closest to what the user had,
/// and a version that cannot read a newer one's file would otherwise replace it on every start.
fn keep_bad(dir: &Path, bytes: &[u8]) {
    for n in 1..=MAX_BAD {
        let path = dir.join(if n == 1 { format!("{FILE_NAME}.bad") } else { format!("{FILE_NAME}.bad.{n}") });
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                if let Err(e) = f.write_all(bytes).and_then(|()| f.sync_all()) {
                    eprintln!("open-controller: could not keep a copy of the settings: {e}");
                }
                return;
            }
            Err(_) if std::fs::read(&path).is_ok_and(|kept| kept == bytes) => return,
            Err(_) => {}
        }
    }
}

/// The file's bytes, `None` when there is none. A file another program holds for a moment (an
/// antivirus or a backup at sign-in) is waited for: read as missing, the first save would
/// replace every assignment with the defaults.
fn read(path: &Path) -> Option<Vec<u8>> {
    let mut tries = 0;
    loop {
        match std::fs::read(path) {
            Ok(bytes) => return Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
            Err(e) if tries == 20 => {
                // Moved aside, so the defaults the app starts with never overwrite it. When even
                // that fails, `save` tries again and leaves the file alone until it can.
                eprintln!("open-controller: could not read the settings: {e}");
                if std::fs::rename(path, path.with_extension("json.unread")).is_err() {
                    UNREAD.store(true, Ordering::Relaxed);
                }
                return None;
            }
            Err(_) => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
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
        assert_eq!(s, Settings { extra: Map::from_iter([("from_a_later_version".into(), 1.into())]), ..Settings::default() });
        let s: Settings = serde_json::from_str(r#"{"hide_originals": false}"#).unwrap();
        assert!(!s.hide_originals);
        assert!(s.mappings.is_empty(), "a file from before mappings has none");
        let s: Settings = serde_json::from_str(
            r#"{"mappings": {"0300be79790000000600000000000000": "0300be79790000000600000000000000,Pad,a:b2,platform:Windows,"}}"#,
        )
        .unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&serde_json::to_string(&s).unwrap()).unwrap().mappings, s.mappings);
        assert_eq!(s.mappings.len(), 1);
    }

    #[test]
    fn a_file_this_version_cannot_read_keeps_what_it_can() {
        let dir = std::env::temp_dir().join(format!("oc-settings-bad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let text = r#"{"hide_originals": false, "language": "Klingon", "later": [1],
            "controllers": {"serial:a": {"list": [{"name": "Racing"}], "active": 0}, "serial:b": {"list": "broken", "native": true},
                "serial:c": 7}}"#;
        std::fs::write(dir.join(FILE_NAME), text).unwrap();
        let s = Settings::load(&dir);
        assert!(!s.hide_originals);
        assert_eq!(s.language, Lang::En);
        assert_eq!(s.extra["later"], serde_json::json!([1]), "what a newer version wrote is kept");
        assert!(s.controllers.contains_key("serial:a"), "{:?}", s.controllers.keys());
        assert!(s.controllers["serial:b"].native, "the rest of a controller's settings is kept");
        assert_eq!(s.controllers["serial:b"].list.len(), 1);
        assert!(!s.controllers.contains_key("serial:c"));
        assert_eq!(std::fs::read_to_string(dir.join("settings.json.bad")).unwrap(), text, "the original is kept");

        // Saved and broken again, by hand or by another version: the first copy stays, and the
        // same file is not copied twice.
        s.save(&dir);
        let again = r#"{"hide_originals": 3}"#;
        for _ in 0..2 {
            std::fs::write(dir.join(FILE_NAME), again).unwrap();
            Settings::load(&dir);
        }
        assert_eq!(std::fs::read_to_string(dir.join("settings.json.bad")).unwrap(), text);
        assert_eq!(std::fs::read_to_string(dir.join("settings.json.bad.2")).unwrap(), again);
        assert!(!dir.join("settings.json.bad.3").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_broken_profile_costs_only_itself() {
        use open_controller_core::binding::{Action, XboxButton};
        let dir = std::env::temp_dir().join(format!("oc-settings-profile-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let text = r#"{"controllers": {"serial:a": {"list": [
            "not a profile",
            {"name": "Racing", "bindings": {"16": {"Xbox": "A"}}, "brightness": 60},
            {"name": "Shooter", "brightness": 900, "programs": ["game.exe"]}
          ], "active": 2, "output": "DualShock4"}}}"#;
        std::fs::write(dir.join(FILE_NAME), text).unwrap();
        let s = Settings::load(&dir);
        assert!(dir.join("settings.json.bad").exists());
        let p = &s.controllers["serial:a"];
        assert_eq!(p.list.len(), 2);
        assert_eq!((p.list[0].name.as_str(), p.list[0].brightness), ("Racing", 60));
        assert_eq!(p.list[0].bindings[&16], Action::Xbox(XboxButton::A));
        assert_eq!(p.list[1].name, "Shooter", "a profile with one unreadable value keeps the others");
        assert_eq!((p.list[1].brightness, p.list[1].programs.as_slice()), (100, ["game.exe".to_string()].as_slice()));
        assert_eq!(p.active().name, "Shooter", "the same profile stays in use");
        assert_eq!(p.output, open_controller_core::profile::Output::DualShock4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_newer_versions_fields_survive_a_save() {
        let dir = std::env::temp_dir().join(format!("oc-settings-newer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let text = r#"{"hide_originals": false, "theme": {"accent": "teal"},
            "controllers": {"serial:a": {"list": [{"name": "Racing", "rumble": 40}], "active": 0, "player_led": 3},
                "serial:b": {"list": [{"name": ""}], "player_led": 1}}}"#;
        std::fs::write(dir.join(FILE_NAME), text).unwrap();
        let s = Settings::load(&dir);
        assert!(!dir.join("settings.json.bad").exists(), "read as it is, nothing salvaged");
        assert!(!s.controllers["serial:b"].is_default(), "a controller with a newer version's settings is kept");
        s.save(&dir);
        let saved: Value = serde_json::from_slice(&std::fs::read(dir.join(FILE_NAME)).unwrap()).unwrap();
        assert_eq!(saved["theme"], serde_json::json!({"accent": "teal"}));
        assert_eq!(saved["controllers"]["serial:a"]["player_led"], 3);
        assert_eq!(saved["controllers"]["serial:a"]["list"][0]["rumble"], 40);
        assert_eq!(saved["controllers"]["serial:b"]["player_led"], 1);
        assert_eq!(Settings::load(&dir), s);
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
    fn a_file_from_0_8_5_loads_the_same() {
        use open_controller_core::binding::{self, Action, XboxButton};
        use open_controller_core::profile::Output;
        let dir = std::env::temp_dir().join(format!("oc-settings-085-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let text = r#"{
  "hide_originals": true,
  "check_updates": true,
  "language": "pt",
  "controllers": {
    "serial:e417d8bc366c": {
      "list": [
        {"name": "", "bindings": {"16": {"Xbox": "A"}, "19": {"Macro": [{"Keys": {"mods": 1, "key": 67}}, {"Wait": 40}]}},
         "light": {"Color": [255, 80, 0]}, "brightness": 60, "low_battery_flash": true,
         "gyro": {"mode": "Off", "sensitivity": 100, "invert_y": false, "aim_sensitivity": null, "y_scale": 100, "toggle": false,
                  "off_button": null, "acceleration": null, "tightening": 0, "anti_deadzone": 12, "tilt": true},
         "sticks": {"deadzone": 8, "anti_deadzone": 0, "dpad": "Off"},
         "pressure": {"left_trigger": null, "right_trigger": null, "keep_buttons": false},
         "programs": ["forzahorizon5.exe"]}
      ],
      "active": 0, "native": false, "model": null, "output": "DualShock4"
    }
  },
  "virtual_driver": "ViGEmBus"
}"#;
        std::fs::write(dir.join(FILE_NAME), text).unwrap();
        let s = Settings::load(&dir);
        assert!(!dir.join("settings.json.bad").exists(), "read as it is, nothing salvaged");
        let p = &s.controllers["serial:e417d8bc366c"];
        assert_eq!(p.output, Output::DualShock4);
        assert_eq!(p.active().bindings[&16], Action::Xbox(XboxButton::A));
        assert!(!binding::remaps_normal(&p.active().bindings), "every normal button is itself");
        s.save(&dir);
        assert_eq!(Settings::load(&dir), s);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remapped_buttons_survive_a_save() {
        use open_controller_core::binding::{self, Action, Chord};
        use open_controller_core::profile::Edit;
        let dir = std::env::temp_dir().join(format!("oc-settings-remap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = Settings::default();
        let p = s.controllers.entry("serial:98b6e9a1c2d3".into()).or_default();
        p.apply(Edit::Letters(true));
        p.apply(Edit::Bind { button: 6, action: Some(Action::Keys(Chord { mods: 0, key: 0x1B })) });
        s.save(&dir);
        let back = Settings::load(&dir);
        assert_eq!(back, s);
        assert!(binding::uses_letters(&back.controllers["serial:98b6e9a1c2d3"].active().bindings));
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
