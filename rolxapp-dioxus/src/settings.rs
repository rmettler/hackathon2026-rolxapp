use rolx_core::storage;
use serde::{Deserialize, Serialize};

/// Where the window was last seen, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

/// Preferences that are kept between runs (`gui.json` in the local data directory).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub show_chart: bool,
    /// Closing the window keeps the app running in the tray (timer, away detection, hotkey).
    pub close_to_tray: bool,
    /// Minutes without input while stamped in before the app asks about it; 0 switches it off.
    pub idle_minutes: i64,
    pub window: Option<WindowGeometry>,
    /// Global shortcut that stamps in or out from anywhere; empty switches it off.
    pub hotkey: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_chart: true,
            close_to_tray: true,
            idle_minutes: rolx_core::idle::DEFAULT_THRESHOLD_MINUTES,
            window: None,
            hotkey: "ctrl+alt+r".into(),
        }
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

    /// Changes one setting without overwriting the others that were saved meanwhile.
    pub fn update(change: impl FnOnce(&mut Settings)) {
        let mut settings = Self::load();
        change(&mut settings);
        settings.save();
    }
}
