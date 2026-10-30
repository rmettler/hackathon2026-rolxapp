use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::paths;

/// An example config file, shown when the file is missing. Keep in sync with `docs/auth-plan.md`.
const EXAMPLE: &str = r#"server_url = "https://rolx.example.com"

# Google OAuth client of type "Desktop app", for the default login in the browser.
[google_desktop_client]
id = "xxxx.apps.googleusercontent.com"
secret = "GOCSPX-..."

# Google OAuth client of type "TVs and Limited Input devices", for `login device`.
[google_device_client]
id = "yyyy.apps.googleusercontent.com"
secret = "GOCSPX-..."
"#;

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
        let text = fs::read_to_string(&path).map_err(|e| {
            let mut msg = format!("cannot read {}: {e}", path.display());
            if e.kind() == std::io::ErrorKind::NotFound {
                msg.push_str(&format!("\ncreate it like this:\n\n{EXAMPLE}"));
            }
            Error::Config(msg)
        })?;
        Self::from_toml(&text)
    }

    /// Parses a config in the format of the config file, e.g. one embedded in a mobile app.
    pub fn from_toml(text: &str) -> Result<Self> {
        Ok(toml::from_str(text)?)
    }

    /// Like [`AppConfig::load`], but the `ROLX_SERVER` environment variable overrides `server_url`.
    pub fn load_with_env() -> Result<Self> {
        let mut config = Self::load()?;
        if let Ok(url) = std::env::var("ROLX_SERVER")
            && !url.trim().is_empty()
        {
            config.server_url = url;
        }
        Ok(config)
    }
}

pub fn config_path() -> Result<PathBuf> {
    let dir = paths::config_dir()
        .ok_or_else(|| Error::Config("cannot determine config directory".into()))?;
    Ok(dir.join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_parses() {
        let config = AppConfig::from_toml(EXAMPLE).unwrap();
        assert!(config.google_desktop_client.is_some());
        assert!(config.google_device_client.is_some());
    }
}
