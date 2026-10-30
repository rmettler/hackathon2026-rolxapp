//! Embeds the RolX config into the app, as a phone has no config file to edit. Uses the file at `ROLX_CONFIG`,
//! else the config of the CLI and desktop app on the build machine (`cargo run -- config path`).

use std::path::PathBuf;
use std::{env, fs};

fn main() {
    println!("cargo::rerun-if-env-changed=ROLX_CONFIG");
    let path = env::var_os("ROLX_CONFIG")
        .map(PathBuf::from)
        .or_else(|| dirs::config_dir().map(|d| d.join("rolx").join("config.toml")));

    // An empty config makes the app show an error instead of failing the build, so `cargo check --workspace` keeps
    // working without a config.
    let text = match &path {
        Some(path) => {
            println!("cargo::rerun-if-changed={}", path.display());
            fs::read_to_string(path).unwrap_or_else(|e| {
                println!(
                    "cargo::warning=no RolX config embedded, cannot read {}: {e}",
                    path.display()
                );
                String::new()
            })
        }
        None => {
            println!("cargo::warning=no RolX config embedded, cannot determine the config dir");
            String::new()
        }
    };
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("config.toml");
    fs::write(out, text).unwrap();
}
