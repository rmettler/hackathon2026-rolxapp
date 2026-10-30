use serde::{Deserialize, Serialize};

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

#[cfg(feature = "fs")]
pub(crate) use file::resolve_dir;
#[cfg(feature = "fs")]
pub use file::{ConfigError, config_path};

#[cfg(feature = "fs")]
mod file {
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;

    use super::AppConfig;

    #[derive(Debug, thiserror::Error)]
    pub enum ConfigError {
        #[error("cannot determine config directory")]
        DirNotFound,

        #[error("cannot read {path}: {source}")]
        Read {
            path: PathBuf,
            source: std::io::Error,
        },

        /// Holds only the line: toml's messages quote values from the file, which contains secrets.
        #[error("cannot parse {path}: invalid syntax or value near line {line}")]
        Parse { path: PathBuf, line: usize },
    }

    impl AppConfig {
        pub fn load() -> Result<Self, ConfigError> {
            let path = config_path()?;
            let text = match fs::read_to_string(&path) {
                Ok(text) => text,
                Err(source) => return Err(ConfigError::Read { path, source }),
            };
            parse(&text).map_err(|line| ConfigError::Parse { path, line })
        }
    }

    /// Parses config text; the error is the line the problem was found at.
    pub(super) fn parse(text: &str) -> Result<AppConfig, usize> {
        toml::from_str(text).map_err(|e: toml::de::Error| {
            let offset = e.span().map_or(0, |span| span.start);
            text[..offset].matches('\n').count() + 1
        })
    }

    pub fn config_path() -> Result<PathBuf, ConfigError> {
        let dir = resolve_dir(env::var_os("ROLX_CONFIG_DIR"), dirs::config_dir())
            .ok_or(ConfigError::DirNotFound)?;
        Ok(dir.join("config.toml"))
    }

    /// Directory holding RolX files: a non-empty `override_dir` is used as is, otherwise `rolx`
    /// below the platform's `default_base`.
    pub(crate) fn resolve_dir(
        override_dir: Option<OsString>,
        default_base: Option<PathBuf>,
    ) -> Option<PathBuf> {
        match override_dir {
            Some(dir) if !dir.is_empty() => Some(PathBuf::from(dir)),
            _ => default_base.map(|base| base.join("rolx")),
        }
    }
}

#[cfg(all(test, feature = "fs"))]
#[path = "config.tests.rs"]
mod tests;
