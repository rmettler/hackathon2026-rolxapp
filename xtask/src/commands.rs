use std::path::{Path, PathBuf};

use crate::secret_scan;

mod check;
mod fetch_server;
mod input;
mod screenshot;
mod tailwind;

#[derive(clap::Subcommand)]
pub enum Commands {
    /// Run the gate every milestone must pass: fmt, secret-scan, clippy, tests, wasm clippy.
    Check,
    /// Fetch the pinned RolX server repo and check out its rev.
    FetchServer,
    /// Look for secrets in the staged diff and the untracked files.
    SecretScan,
    /// Regenerate the Tailwind CSS of rolx-ui.
    Tailwind,
    /// Save a PNG of a window.
    Screenshot {
        /// Part of the window title
        #[arg(long)]
        window: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// Send mouse or keyboard input.
    Input {
        #[command(subcommand)]
        action: input::InputAction,
    },
}

impl Commands {
    pub fn run(&self, root: &Path) -> Result<String, String> {
        match self {
            Commands::Check => check::run(root),
            Commands::FetchServer => fetch_server::run(root),
            Commands::SecretScan => secret_scan::run(root),
            Commands::Tailwind => tailwind::run(root),
            Commands::Screenshot { window, out } => screenshot::run(window, out),
            Commands::Input { action } => input::run(action),
        }
    }
}
