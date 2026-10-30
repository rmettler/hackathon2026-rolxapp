use std::path::Path;

mod checkout;

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
}

impl Commands {
    pub fn run(&self, root: &Path) -> Result<String, String> {
        match self {
            Commands::FetchServer => checkout::run(root, &SERVER),
            Commands::FetchDesignSystem => checkout::run(root, &DESIGN_SYSTEM),
        }
    }
}
