//! Compiles the app's settings in: an Android app has no config file a user could edit. Each comes
//! from the environment, else from the gitignored `config.local.toml`:
//!
//! ```toml
//! server_url = "http://10.0.2.2:8090"
//! google_device_client_id = "..."
//! google_device_client_secret = "..."
//! google_base_url = "http://10.0.2.2:8090/google" # debug builds only
//! ```

use std::env;
use std::fs;

const SETTINGS: [(&str, &str); 4] = [
    ("server_url", "ROLX_SERVER_URL"),
    ("google_device_client_id", "ROLX_GOOGLE_DEVICE_CLIENT_ID"),
    (
        "google_device_client_secret",
        "ROLX_GOOGLE_DEVICE_CLIENT_SECRET",
    ),
    ("google_base_url", "ROLX_ANDROID_GOOGLE_BASE_URL"),
];

fn main() {
    println!("cargo:rerun-if-changed=config.local.toml");

    let file: toml::Table = fs::read_to_string("config.local.toml")
        .ok()
        .and_then(|text| {
            let parsed = text.parse().ok();
            if parsed.is_none() {
                // Without the details: they would quote the file.
                println!("cargo:warning=config.local.toml could not be parsed and is ignored");
            }
            parsed
        })
        .unwrap_or_default();

    for (key, variable) in SETTINGS {
        println!("cargo:rerun-if-env-changed={variable}");
        let from_file = file
            .get(key)
            .and_then(|value| value.as_str())
            .map(str::to_string);
        let from_env = env::var(variable).ok().filter(|value| !value.is_empty());
        if let Some(value) = from_env.or(from_file) {
            println!("cargo:rustc-env={variable}={value}");
        }
    }
}
