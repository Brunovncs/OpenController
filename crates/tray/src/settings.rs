//! Preferences, kept in `settings.json` in the app data folder.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const FILE_NAME: &str = "settings.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Hide the original controllers from games while Open Controller runs (needs HidHide).
    pub hide_originals: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { hide_originals: true }
    }
}

impl Settings {
    pub fn load(dir: &Path) -> Settings {
        std::fs::read(dir.join(FILE_NAME)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) {
        if let Ok(text) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(dir.join(FILE_NAME), text);
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
}
