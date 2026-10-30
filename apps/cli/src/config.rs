use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientCredentials {
    pub id: String,
    pub secret: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub server_url: String,
    pub google_desktop_client: Option<ClientCredentials>,
    pub google_device_client: Option<ClientCredentials>,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        let text = fs::read_to_string(&path)
            .map_err(|e| Error::Config(format!("cannot deserialize {}: {}", path.display(), e)))?;
        Ok(toml::from_str(&text)?)
    }
}

pub fn config_path() -> Result<PathBuf> {
    let dir = resolve_dir(env::var_os("ROLX_CONFIG_DIR"), dirs::config_dir())
        .ok_or_else(|| Error::Config("cannot determine config directory".into()))?;
    Ok(dir.join("config.toml"))
}

/// Directory holding RolX files: a non-empty `override_dir` is used as is, otherwise `rolx` below
/// the platform's `default_base`.
pub fn resolve_dir(
    override_dir: Option<OsString>,
    default_base: Option<PathBuf>,
) -> Option<PathBuf> {
    match override_dir {
        Some(dir) if !dir.is_empty() => Some(PathBuf::from(dir)),
        _ => default_base.map(|base| base.join("rolx")),
    }
}

#[cfg(test)]
#[path = "config.tests.rs"]
mod tests;
