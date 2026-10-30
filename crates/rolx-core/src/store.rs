use crate::session::Session;

#[cfg(feature = "fs")]
mod file;

#[cfg(feature = "fs")]
pub use file::FileSessionStore;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("cannot determine cache directory")]
    CacheDirNotFound,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("session format error: {0}")]
    Format(#[from] serde_json::Error),

    /// For stores whose backend has its own error type, e.g. the browser's local storage.
    #[error("session store error: {0}")]
    Backend(String),
}

/// Where a platform keeps the session between runs.
pub trait SessionStore {
    /// Returns `None` if no session is stored.
    fn load(&self) -> Result<Option<Session>, StoreError>;

    fn save(&self, session: &Session) -> Result<(), StoreError>;

    /// Succeeds if no session is stored.
    fn delete(&self) -> Result<(), StoreError>;
}
