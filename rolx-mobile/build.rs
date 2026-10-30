//! Embeds the RolX config into the app, as a phone has no config file to edit. Uses the file at `ROLX_CONFIG`,
//! else the config of the CLI and desktop app on the build machine (`cargo run -- config path`).
//!
//! Also embeds the Cudos brand font (KMR Apparat) as `@font-face` rules when its files are available: from
//! `ROLX_FONT_DIR`, else `public/fonts` of the `./design-system` checkout (`cargo xtask fetch-design-system`). The
//! files are licensed separately and never committed; without them the app falls back to Helvetica Neue / Arial.

use std::path::{Path, PathBuf};
use std::{env, fs};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

/// (CSS weight, file name), as in the desktop app's `fonts.rs`.
const FONT_FACES: [(u16, &str); 3] = [
    (400, "KMR-Apparat-Regular.woff2"),
    (500, "KMR-Apparat-Medium.woff2"),
    (700, "KMR-Apparat-Bold.woff2"),
];

fn main() {
    embed_config();
    embed_fonts();
}

fn embed_fonts() {
    println!("cargo::rerun-if-env-changed=ROLX_FONT_DIR");
    let dir = env::var_os("ROLX_FONT_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../design-system/public/fonts"),
        PathBuf::from,
    );
    let css: String = FONT_FACES
        .iter()
        .filter_map(|(weight, file)| {
            let path = dir.join(file);
            println!("cargo::rerun-if-changed={}", path.display());
            let bytes = fs::read(&path).ok()?;
            Some(format!(
                "@font-face {{ font-family: 'KMR Apparat'; font-style: normal; font-weight: {weight}; font-display: swap; src: url(data:font/woff2;base64,{}) format('woff2'); }}
",
                STANDARD.encode(bytes)
            ))
        })
        .collect();
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("fonts.css");
    fs::write(out, css).unwrap();
}

fn embed_config() {
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
