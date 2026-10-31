//! The TUI's local data as JSON files: the local copy, the outbox and an instance lock.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

#[derive(Clone)]
pub struct LocalStore {
    root: PathBuf,
}

/// Held for as long as this TUI instance runs; dropping it releases the lock.
pub struct InstanceLock {
    _file: File,
}

impl LocalStore {
    pub fn at(root: PathBuf) -> Self {
        Self { root }
    }

    /// The TUI's own directory inside the user's data directory.
    pub fn default_location() -> Option<Self> {
        dirs::data_local_dir().map(|dir| Self::at(dir.join("rolxapp-tui")))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Reads `name` below the root; a missing file is `None`.
    pub fn read<T: DeserializeOwned>(&self, name: &str) -> Result<Option<T>, String> {
        let path = self.root.join(name);
        match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text)
                .map(Some)
                .map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// Writes through a temporary file so a crash never leaves a half-written file.
    pub fn write<T: Serialize>(&self, name: &str, value: &T) -> Result<(), String> {
        let path = self.root.join(name);
        let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let temporary = path.with_extension("json.tmp");
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|e| format!("{}: {e}", temporary.display()))?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&temporary, &path).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Deletes `name`; a file that is not there is fine.
    pub fn remove(&self, name: &str) -> Result<(), String> {
        let path = self.root.join(name);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// The `.json` files directly inside `dir`, by name.
    pub fn list(&self, dir: &str) -> Result<Vec<String>, String> {
        let path = self.root.join(dir);
        let entries = match fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|name| name.ends_with(".json"))
            .collect();
        names.sort();
        Ok(names)
    }

    /// Takes the exclusive lock, so a second TUI cannot corrupt the same files.
    pub fn lock(&self) -> Result<InstanceLock, String> {
        fs::create_dir_all(&self.root).map_err(|e| format!("{}: {e}", self.root.display()))?;
        let path = self.root.join("lock");
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        match file.try_lock() {
            Ok(()) => Ok(InstanceLock { _file: file }),
            Err(TryLockError::WouldBlock) => Err(format!(
                "RolX TUI is already running (lock file {})",
                path.display()
            )),
            Err(TryLockError::Error(e)) => Err(format!("{}: {e}", path.display())),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;
    use crate::test_support::temp_local_store;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Sample {
        value: i32,
    }

    #[test]
    fn a_missing_file_reads_as_none() {
        let store = temp_local_store();

        assert_eq!(store.read::<Sample>("nothing.json"), Ok(None));
    }

    #[test]
    fn writes_and_reads_back_in_subdirectories() {
        let store = temp_local_store();

        store
            .write("records/2026-10.json", &Sample { value: 7 })
            .unwrap();

        assert_eq!(
            store.read::<Sample>("records/2026-10.json"),
            Ok(Some(Sample { value: 7 }))
        );
        assert_eq!(store.list("records").unwrap(), ["2026-10.json"]);
        assert!(store.list("elsewhere").unwrap().is_empty());
    }

    #[test]
    fn a_damaged_file_is_an_error_and_is_left_alone() {
        let store = temp_local_store();
        store.write("outbox.json", &Sample { value: 1 }).unwrap();
        fs::write(store.root().join("outbox.json"), "not json").unwrap();

        assert!(store.read::<Sample>("outbox.json").is_err());
        assert_eq!(
            fs::read_to_string(store.root().join("outbox.json")).unwrap(),
            "not json"
        );
    }

    #[test]
    fn a_second_instance_cannot_take_the_lock() {
        let store = temp_local_store();

        let first = store.lock().unwrap();
        let second = store.lock();

        assert!(second.err().unwrap().contains("already running"));
        drop(first);
        assert!(store.lock().is_ok(), "released with the first instance");
    }
}
