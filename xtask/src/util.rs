use std::path::Path;
use std::process::{Command, Output};

pub fn run_cmd(name: &str, args: &[&str], dir: &Path) -> Result<Output, String> {
    Command::new(name)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("failed to run `{name}`: {e}"))
}

pub fn check_cmd_output(out: &Output, command: &str) -> Result<(), String> {
    out.status.success().ok_or_else(|| {
        format!(
            "{command} failed (exit {:?}):\n{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

/// Top-level document returned by `cargo metadata`.
#[derive(serde::Deserialize)]
pub struct CargoMetadata {
    pub packages: Vec<CargoPackage>,
}

/// A single package entry from `cargo metadata`.
#[derive(serde::Deserialize)]
pub struct CargoPackage {
    pub name: String,
    pub version: String,
    /// `[package.metadata]` — free-form; schema varies per package.
    pub metadata: serde_json::Value,
}

/// Run `cargo metadata --no-deps` and parse the output.
pub fn read_cargo_metadata(root: &Path) -> Result<CargoMetadata, String> {
    let out = run_cmd(
        "cargo",
        &["metadata", "--no-deps", "--format-version", "1"],
        root,
    )?;
    check_cmd_output(&out, "`cargo metadata`")?;

    serde_json::from_slice(&out.stdout).map_err(|e| format!("failed to parse cargo metadata: {e}"))
}
