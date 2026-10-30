//! Remembering the last change made to a day, so it can be taken back.
//!
//! Only one step is kept: what the day looked like before and after the change. Undoing writes
//! `before` back, but only if the day on the server still is `after` (see `dayedit::undo_last`).

use std::path::{Path, PathBuf};

use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};

use crate::dayplan::DayEntry;
use crate::storage;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UndoRecord {
    pub day: NaiveDate,
    /// What happened, e.g. `Deleted #2`.
    pub what: String,
    pub before: Vec<DayEntry>,
    pub after: Vec<DayEntry>,
    pub at: NaiveDateTime,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UndoStore {
    pub last: Option<UndoRecord>,
}

impl UndoStore {
    pub fn default_path() -> anyhow::Result<PathBuf> {
        storage::data_file("undo.json")
    }

    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        storage::read_json_or_default(path)
    }

    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        storage::write_json(path, self)
    }

    pub fn load() -> anyhow::Result<Self> {
        Self::load_from(&Self::default_path()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_means_nothing_to_undo() {
        let path = std::env::temp_dir().join("rolx-undo-test-does-not-exist.json");
        assert_eq!(UndoStore::load_from(&path).unwrap().last, None);
    }

    #[test]
    fn the_record_survives_a_round_trip() {
        let path = std::env::temp_dir().join(format!("rolx-undo-test-{}.json", std::process::id()));
        let store = UndoStore {
            last: Some(UndoRecord {
                day: NaiveDate::from_ymd_opt(2026, 10, 30).unwrap(),
                what: "Deleted #1".into(),
                before: vec![DayEntry {
                    activity_id: 1,
                    full_activity_number: "A-1".into(),
                    begin: Some(28_800),
                    duration: 3_600,
                    pause: None,
                    comment: Some("x".into()),
                }],
                after: Vec::new(),
                at: NaiveDate::from_ymd_opt(2026, 10, 30)
                    .unwrap()
                    .and_hms_opt(9, 0, 0)
                    .unwrap(),
            }),
        };
        store.save_to(&path).unwrap();
        let loaded = UndoStore::load_from(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(loaded, store);
    }
}
