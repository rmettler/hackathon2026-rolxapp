use std::fs::{self, File};
use std::sync::Mutex;

use tracing_subscriber::EnvFilter;

fn filter(verbosity: u8) -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let level = match verbosity {
            0 => "warn",
            1 => "info",
            _ => "debug",
        };

        EnvFilter::new(level)
    })
}

pub fn init(verbosity: u8) {
    if let Err(err) = tracing_subscriber::fmt()
        .with_env_filter(filter(verbosity))
        .try_init()
    {
        eprintln!("failed to initialize logging: {err}");
    }
}

/// Logs to `<cache dir>/rolx/<name>` instead of stderr, for frontends that own the terminal.
pub fn init_file(verbosity: u8, name: &str) -> anyhow::Result<()> {
    let dir = dirs::cache_dir()
        .ok_or_else(|| anyhow::anyhow!("cache directory not found"))?
        .join("rolx");
    fs::create_dir_all(&dir)?;
    let file = File::create(dir.join(name))?;
    tracing_subscriber::fmt()
        .with_env_filter(filter(verbosity))
        .with_ansi(false)
        .with_writer(Mutex::new(file))
        .try_init()
        .map_err(|e| anyhow::anyhow!("failed to initialize logging: {e}"))
}

/// Redacts the values of given `sensitive_keys` if direct key of `value`. Disabled for debug builds.
#[cfg(not(debug_assertions))]
pub fn redact_json(mut value: serde_json::Value, sensitive_keys: &[&str]) -> serde_json::Value {
    if let Some(obj) = value.as_object_mut() {
        for &key in sensitive_keys {
            if obj.contains_key(key) {
                obj.insert(key.into(), serde_json::Value::String("***".into()));
            }
        }
    }
    value
}

/// Returns value as is for non-debug builds.
#[cfg(debug_assertions)]
pub fn redact_json(value: serde_json::Value, _sensitive_keys: &[&str]) -> serde_json::Value {
    value
}
