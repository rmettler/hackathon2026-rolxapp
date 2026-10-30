use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BearerToken(String);

impl BearerToken {
    pub fn from_string(s: String) -> Self {
        BearerToken(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub bearer_token: BearerToken,
    pub expires: DateTime<Utc>,
    pub user_id: String,
    pub user_email: String,
    pub user_first_name: String,
    pub user_last_name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error(transparent)]
    CacheDirNotFound(#[from] CacheDirNotFoundError),

    #[error("no session found")]
    NoSession,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("deserialization error: {0}")]
    Deserialize(#[from] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error(transparent)]
    CacheDirNotFound(#[from] CacheDirNotFoundError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum DeletionError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    CacheDirNotFound(#[from] CacheDirNotFoundError),
}

#[derive(Debug, thiserror::Error)]
#[error("cannot determine cache directory")]
pub struct CacheDirNotFoundError {}

impl Session {
    pub fn is_near_expiry(&self) -> bool {
        self.expires < Utc::now() + Duration::minutes(10)
    }

    pub fn is_expired(&self) -> bool {
        self.expires < Utc::now()
    }

    pub fn load() -> std::result::Result<Self, LoadError> {
        let path = session_path()?;
        if !path.exists() {
            return Err(LoadError::NoSession);
        }
        let text = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn save(&self) -> std::result::Result<(), SaveError> {
        let path = session_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&path)?
                .write_all(text.as_bytes())?;
        }
        #[cfg(not(unix))]
        fs::write(&path, text.as_bytes())?;
        Ok(())
    }

    pub fn delete() -> std::result::Result<(), DeletionError> {
        let path = session_path()?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}

fn session_path() -> std::result::Result<PathBuf, CacheDirNotFoundError> {
    dirs::cache_dir()
        .map(|p| p.join("rolx").join("session.json"))
        .ok_or(CacheDirNotFoundError {})
}
