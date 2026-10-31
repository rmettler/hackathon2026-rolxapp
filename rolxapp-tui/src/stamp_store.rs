//! The running stamp, kept in a file so it survives closing the TUI.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct StampFile {
    started: DateTime<Local>,
}

pub struct StampStore {
    path: PathBuf,
}

impl StampStore {
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    /// The file in the user's data directory, owned by the TUI alone.
    pub fn default_location() -> Option<Self> {
        dirs::data_local_dir().map(|dir| Self::at(dir.join("rolxapp-tui").join("stamp.json")))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The start of the running stamp, or `None` when there is none.
    pub fn load(&self) -> Result<Option<DateTime<Local>>, String> {
        match fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str::<StampFile>(&text)
                .map(|file| Some(file.started))
                .map_err(|e| e.to_string()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Writes through a temporary file so a crash never leaves a half-written stamp.
    pub fn save(&self, started: DateTime<Local>) -> Result<(), String> {
        let text = serde_json::to_string(&StampFile { started }).map_err(|e| e.to_string())?;
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let temporary = self.path.with_extension("json.tmp");
        fs::write(&temporary, text).map_err(|e| e.to_string())?;
        fs::rename(&temporary, &self.path).map_err(|e| e.to_string())
    }

    pub fn clear(&self) -> Result<(), String> {
        match fs::remove_file(&self.path) {
            Err(e) if e.kind() != ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::test_support::temp_store;

    fn started() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 30, 9, 12, 40).unwrap()
    }

    #[test]
    fn a_missing_file_means_no_stamp() {
        assert_eq!(temp_store().load(), Ok(None));
    }

    #[test]
    fn saves_and_loads_the_start() {
        let store = temp_store();

        store.save(started()).unwrap();

        assert_eq!(store.load(), Ok(Some(started())));
    }

    #[test]
    fn clearing_removes_the_stamp_and_is_repeatable() {
        let store = temp_store();
        store.save(started()).unwrap();

        store.clear().unwrap();
        store.clear().unwrap();

        assert_eq!(store.load(), Ok(None));
    }

    #[test]
    fn a_damaged_file_is_an_error_and_is_left_alone() {
        let store = temp_store();
        store.save(started()).unwrap();
        fs::write(store.path(), "not json").unwrap();

        assert!(store.load().is_err());
        assert_eq!(fs::read_to_string(store.path()).unwrap(), "not json");
    }
}
