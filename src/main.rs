mod cli;
mod commands;

use clap::Parser;
use cli::{Cli, Command, ConfigCommand, LoginFlow};
use rolx_core::client::RolxClient;
use rolx_core::config::AppConfig;
use rolx_core::login::AuthFlow;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    rolx_core::logging::init(cli.verbose);

    match cli.command {
        Command::Login { flow } => {
            let auth_flow = match flow {
                Some(LoginFlow::NativeApp) => AuthFlow::NativeApp,
                Some(LoginFlow::Device) => AuthFlow::Device,
                None => AuthFlow::NativeApp,
            };
            let config = load_config(cli.server)?;
            rolx_core::login::run(&config, auth_flow).await?;
            Ok(())
        }
        Command::Logout => commands::logout::run(),
        Command::Whoami => commands::whoami::run(),
        Command::Records { date } => {
            let config = load_config(cli.server)?;
            let client = RolxClient::new(&config).await?;
            commands::records::run(&client, date).await
        }
        Command::Config { command } => match command {
            ConfigCommand::Ls => commands::config::ls(),
            ConfigCommand::Path => commands::config::path(),
        },
        Command::Stamp { command } => commands::stamp::run(command),
        Command::Sync => {
            let config = load_config(cli.server)?;
            commands::sync::run(&config).await
        }
        Command::Activities {
            words,
            favourites,
            recent,
            refresh,
        } => {
            if refresh {
                let config = load_config(cli.server)?;
                let client = RolxClient::new(&config).await?;
                let loaded =
                    rolx_core::activities::refresh(&client, chrono::Local::now().date_naive())
                        .await?;
                println!("Loaded {} activities", loaded.len());
            }
            commands::activities::run(words, favourites, recent)
        }
        Command::Favourite { activity, remove } => {
            let config = load_config(cli.server)?;
            commands::favourite::run(&config, activity, remove).await
        }
        Command::Entry { command } => {
            let config = load_config(cli.server)?;
            commands::entry::run(&config, command).await
        }
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
