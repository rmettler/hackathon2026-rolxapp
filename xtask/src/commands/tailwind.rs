use std::fs;
use std::path::{Path, PathBuf};

use crate::util::{check_cmd_output, run_cmd};

/// The version `dx` 0.8.0-alpha.3 installs, so `dx` and this command generate the same CSS.
const VERSION: &str = "v4.1.5";

pub const CSS_FILE: &str = "crates/rolx-ui/assets/tailwind.css";
const INPUT_FILE: &str = "crates/rolx-ui/tailwind.css";

/// Regenerates the CSS the way `dx` does (`Dioxus.toml` of the apps points it at the same files).
pub fn run(root: &Path) -> Result<String, String> {
    let binary = find_binary()?;
    let out = run_cmd(
        &binary.to_string_lossy(),
        &["--input", INPUT_FILE, "--output", CSS_FILE],
        root,
    )?;
    check_cmd_output(&out, "tailwindcss")?;
    Ok(format!("tailwind: generated {CSS_FILE}"))
}

/// The binary `dx` installed below `~/.dx/tools`.
fn find_binary() -> Result<PathBuf, String> {
    let tools = dirs::home_dir()
        .ok_or("cannot determine the home directory")?
        .join(".dx")
        .join("tools");
    let dir = tools.join(format!("tailwindcss-{VERSION}"));
    let binary = fs::read_dir(&dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.is_file()
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("tailwindcss"))
        });
    binary.ok_or_else(|| {
        format!(
            "tailwindcss {VERSION} not found in {}; `dx build -p rolxapp-desktop --renderer native` installs it",
            dir.display()
        )
    })
}
