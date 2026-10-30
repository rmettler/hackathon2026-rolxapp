use std::path::Path;

mod android;
mod checkout;
mod fake_server;
mod fake_session;

use checkout::Target;

const SERVER: Target = Target {
    key: "rolx-server",
    label: "RolX server",
};

const DESIGN_SYSTEM: Target = Target {
    key: "design-system",
    label: "Cudos design system",
};

#[derive(clap::Subcommand)]
pub enum Commands {
    /// Fetch the pinned RolX server repo and check out its rev.
    FetchServer,
    /// Fetch the pinned Cudos design system repo and check out its rev.
    FetchDesignSystem,
    /// Run an in-memory fake RolX server with seeded data, to test the apps without the real one.
    FakeServer(fake_server::FakeServerArgs),
    /// Write a session.json for the fake server's user, so an app starts logged in.
    FakeSession(fake_session::FakeSessionArgs),
    /// Android dev helpers for the mobile app (emulator, devices, run, apk, push-session).
    #[command(subcommand)]
    Android(android::AndroidCommand),
}

impl Commands {
    pub fn run(&self, root: &Path) -> Result<String, String> {
        match self {
            Commands::FetchServer => checkout::run(root, &SERVER),
            Commands::FetchDesignSystem => checkout::run(root, &DESIGN_SYSTEM),
            Commands::FakeServer(args) => fake_server::run(args),
            Commands::FakeSession(args) => fake_session::run(args),
            Commands::Android(cmd) => cmd.run(root),
        }
    }
}
