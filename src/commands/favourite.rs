use rolx_core::activities;
use rolx_core::client::RolxClient;
use rolx_core::config::AppConfig;
use rolx_core::stamp::StampState;

use super::pick::resolve_activity;

pub async fn run(config: &AppConfig, words: Vec<String>, remove: bool) -> anyhow::Result<()> {
    let usage = StampState::load()?.usage;
    let activity = resolve_activity(&words, &usage)?;

    let client = RolxClient::new(config).await?;
    activities::set_favourite(&client, &activity, !remove).await?;

    println!(
        "{} {} {}",
        if remove { "Removed" } else { "Added" },
        activity.label(),
        if remove {
            "from favourites"
        } else {
            "to favourites"
        }
    );
    Ok(())
}
