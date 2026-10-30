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
    let base = dirs::config_dir()
        .ok_or_else(|| Error::Config("cannot determine config directory".into()))?;
    Ok(base.join("rolx").join("config.toml"))
}
