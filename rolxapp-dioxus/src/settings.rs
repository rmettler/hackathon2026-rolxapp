use rolx_core::storage;
use serde::{Deserialize, Serialize};

/// Preferences that are kept between runs (`gui.json` in the local data directory).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub show_chart: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { show_chart: true }
    }
}

impl Settings {
    pub fn load() -> Self {
        storage::data_file("gui.json")
            .and_then(|path| storage::read_json_or_default(&path))
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(path) = storage::data_file("gui.json") {
            let _ = storage::write_json(&path, self);
        }
    }
}
