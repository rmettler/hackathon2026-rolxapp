use std::path::Path;

mod android;
mod fetch_server;

#[derive(clap::Subcommand)]
pub enum Commands {
    /// Fetch the pinned RolX server repo and check out its rev.
    FetchServer,
    /// Android dev helpers for the mobile app (emulator, devices, run, apk).
    #[command(subcommand)]
    Android(android::AndroidCommand),
}

impl Commands {
    pub fn run(&self, root: &Path) -> Result<String, String> {
        match self {
            Commands::FetchServer => fetch_server::run(root),
            Commands::Android(cmd) => cmd.run(root),
        }
    }
}
