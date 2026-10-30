//! Small helpers for the app's local data files (stamp queue, activity cache).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Path of a file in the per-user data directory (`<data dir>/rolx/<name>`).
/// The `ROLX_DATA_DIR` environment variable replaces the directory (`$ROLX_DATA_DIR/<name>`).
pub fn data_file(name: &str) -> anyhow::Result<PathBuf> {
    if let Some(dir) = std::env::var_os("ROLX_DATA_DIR") {
        return Ok(PathBuf::from(dir).join(name));
    }
    let base = dirs::data_dir().ok_or_else(|| anyhow!("cannot determine data directory"))?;
    Ok(base.join("rolx").join(name))
}

/// Reads a JSON file, returning the default value if it doesn't exist yet.
/// A file that exists but cannot be parsed is an error, so it is never silently overwritten.
pub fn read_json_or_default<T: DeserializeOwned + Default>(path: &Path) -> anyhow::Result<T> {
    match fs::read_to_string(path) {
        Ok(text) => {
            serde_json::from_str(&text).with_context(|| format!("cannot parse {}", path.display()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
    }
}

/// Writes a JSON file via a temporary file, so a crash cannot leave a half-written file behind.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(value)?)
        .with_context(|| format!("cannot write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("cannot replace {}", path.display()))?;
    Ok(())
}
