use std::fs;
use std::path::PathBuf;

use super::{SessionStore, StoreError};
use crate::config::resolve_dir;
use crate::session::Session;

/// Keeps the session as a JSON file.
pub struct FileSessionStore {
    path: PathBuf,
}

impl FileSessionStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// `session.json` in `ROLX_CACHE_DIR` if set, else below the platform's cache directory.
    pub fn default_location() -> Result<Self, StoreError> {
        resolve_dir(std::env::var_os("ROLX_CACHE_DIR"), dirs::cache_dir())
            .map(|dir| Self::new(dir.join("session.json")))
            .ok_or(StoreError::CacheDirNotFound)
    }
}

impl SessionStore for FileSessionStore {
    fn load(&self) -> Result<Option<Session>, StoreError> {
        if !self.path.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(&self.path)?;
        Ok(Some(serde_json::from_str(&text)?))
    }

    fn save(&self, session: &Session) -> Result<(), StoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(session)?;
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&self.path)?
                .write_all(text.as_bytes())?;
        }
        #[cfg(not(unix))]
        fs::write(&self.path, text.as_bytes())?;
        Ok(())
    }

    fn delete(&self) -> Result<(), StoreError> {
        if self.path.exists() {
            fs::remove_file(&self.path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::session::BearerToken;

    fn session() -> Session {
        Session {
            bearer_token: BearerToken::from_string("fake".into()),
            expires: Utc.with_ymd_and_hms(2030, 1, 2, 3, 4, 5).unwrap(),
            user_id: "id".into(),
            user_email: "test@example.invalid".into(),
            user_first_name: "Test".into(),
            user_last_name: "User".into(),
        }
    }

    #[test]
    fn round_trip_creates_missing_directories() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileSessionStore::new(dir.path().join("sub").join("session.json"));

        store.save(&session()).unwrap();
        let loaded = store.load().unwrap().unwrap();

        assert_eq!(loaded.bearer_token.as_str(), "fake");
        assert_eq!(loaded.expires, session().expires);
        assert_eq!(loaded.user_id, "id");
        assert_eq!(loaded.user_email, "test@example.invalid");
        assert_eq!(loaded.user_first_name, "Test");
        assert_eq!(loaded.user_last_name, "User");
    }

    #[test]
    fn missing_file_loads_as_none_and_deletes_fine() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileSessionStore::new(dir.path().join("session.json"));

        assert!(store.load().unwrap().is_none());
        store.delete().unwrap();
    }

    #[test]
    fn delete_removes_the_session() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileSessionStore::new(dir.path().join("session.json"));
        store.save(&session()).unwrap();

        store.delete().unwrap();

        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn malformed_file_is_a_format_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        fs::write(&path, "not json").unwrap();

        let result = FileSessionStore::new(path).load();

        assert!(matches!(result, Err(StoreError::Format(_))));
    }
}
