mod cli;
mod commands;

use clap::Parser;
use cli::{Cli, Command, ConfigCommand, LoginFlow};
use rolx_core::client::RolxClient;
use rolx_core::config::AppConfig;
use rolx_core::login::{AuthFlow, UserAction};

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
                Some(LoginFlow::NativeApp) => AuthFlow::NativeApp { return_url: None },
                Some(LoginFlow::Device) => AuthFlow::Device,
                None => AuthFlow::NativeApp { return_url: None },
            };
            let config = load_config(cli.server)?;
            rolx_core::login::run(&config, auth_flow, |action| match action {
                UserAction::OpenUrl { url } => {
                    println!("Opening browser failed!\n\nOpen this URL in your browser:\n  {url}")
                }
                UserAction::EnterCode { url, code } => {
                    println!("To authorize this application, visit:\n  {url}");
                    println!("Then enter the following code: {code}\n");
                }
            })
            .await?;
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
