//! Directories for the config, session and local data files.

use std::path::PathBuf;
use std::sync::OnceLock;

static BASE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Stores config, session and data files in `dir` instead of the OS's config, cache and data dirs. Required on Android, where the
/// OS dirs are unknown; pass the app's files dir. Must be called before loading or saving anything; returns
/// `dir` back as error if a base dir was already set.
pub fn set_base_dir(dir: PathBuf) -> Result<(), PathBuf> {
    BASE_DIR.set(dir)
}

/// Returns the directory of the config file.
pub fn config_dir() -> Option<PathBuf> {
    BASE_DIR
        .get()
        .cloned()
        .or_else(|| dirs::config_dir().map(|p| p.join("rolx")))
}

/// Returns the directory of the session file.
pub fn cache_dir() -> Option<PathBuf> {
    BASE_DIR
        .get()
        .cloned()
        .or_else(|| dirs::cache_dir().map(|p| p.join("rolx")))
}

/// Returns the directory of the local data files (stamp queue, activity cache, ...).
pub fn data_dir() -> Option<PathBuf> {
    BASE_DIR
        .get()
        .cloned()
        .or_else(|| dirs::data_dir().map(|p| p.join("rolx")))
}
