mod cli;
mod commands;
mod logging;

use clap::Parser;
use cli::{Cli, Command, ConfigCommand, LoginFlow};
use commands::login::AuthFlow;
use rolx_core::client::RolxClient;
use rolx_core::config::AppConfig;
use rolx_core::store::FileSessionStore;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    logging::init(cli.verbose);

    match cli.command {
        Command::Login { flow } => {
            let auth_flow = match flow {
                Some(LoginFlow::NativeApp) => AuthFlow::NativeApp,
                Some(LoginFlow::Device) => AuthFlow::Device,
                None => AuthFlow::NativeApp,
            };
            let config = load_config(cli.server)?;
            commands::login::run(&config, auth_flow).await
        }
        Command::Logout => commands::logout::run(),
        Command::Whoami => commands::whoami::run(),
        Command::Records { date } => {
            let config = load_config(cli.server)?;
            let store = FileSessionStore::default_location()?;
            let client =
                RolxClient::from_store(reqwest::Client::new(), config.server_url, &store).await?;
            commands::records::run(&client, date).await
        }
        Command::Config { command } => match command {
            ConfigCommand::Ls => commands::config::ls(),
            ConfigCommand::Path => commands::config::path(),
        },
    }
}

/// Loads the config file, applying the `--server` override.
fn load_config(server: Option<String>) -> anyhow::Result<AppConfig> {
    let mut config = AppConfig::load()?;
    if let Some(server) = server {
        config.server_url = server;
    }
    Ok(config)
}
