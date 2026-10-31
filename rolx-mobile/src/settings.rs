use rolx_core::storage;
use serde::{Deserialize, Serialize};

const FILE: &str = "mobile.json";

/// Preferences that are kept between runs (`mobile.json` in the app's data dir; the desktop app has `gui.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub show_chart: bool,
    /// Ask for the fingerprint, face or screen lock when opening the app, see `app_lock.rs`.
    pub app_lock: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_chart: true,
            app_lock: false,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        storage::data_file(FILE)
            .and_then(|path| storage::read_json_or_default(&path))
            .unwrap_or_default()
    }

    /// Changes one setting without overwriting the others that were saved meanwhile.
    pub fn update(change: impl FnOnce(&mut Settings)) {
        let mut settings = Self::load();
        change(&mut settings);
        if let Ok(path) = storage::data_file(FILE) {
            let _ = storage::write_json(&path, &settings);
        }
    }
}
