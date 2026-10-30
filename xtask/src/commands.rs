use std::path::Path;

use crate::secret_scan;

mod check;
mod fetch_server;

#[derive(clap::Subcommand)]
pub enum Commands {
    /// Run the gate every milestone must pass: fmt, secret-scan, clippy, tests, wasm clippy.
    Check,
    /// Fetch the pinned RolX server repo and check out its rev.
    FetchServer,
    /// Look for secrets in the staged diff and the untracked files.
    SecretScan,
}

impl Commands {
    pub fn run(&self, root: &Path) -> Result<String, String> {
        match self {
            Commands::Check => check::run(root),
            Commands::FetchServer => fetch_server::run(root),
            Commands::SecretScan => secret_scan::run(root),
        }
    }
}
