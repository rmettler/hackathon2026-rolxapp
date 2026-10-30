//! Helpers shared by the GUI components.

use rolx_core::activities::{self, Activity};
use rolx_core::client::RolxClient;
use rolx_core::session::{LoadError, Session};

/// A client for the logged-in user. Fails if there is no valid session.
pub async fn client() -> anyhow::Result<RolxClient> {
    let config = crate::config()?;
    RolxClient::new(&config).await
}

/// Whether there is a session that is not expired. Stamping works without one.
pub fn has_session() -> bool {
    match Session::load() {
        Err(LoadError::NoSession) => false,
        Ok(session) => !session.is_expired(),
        Err(_) => true,
    }
}

pub async fn set_favourite(activity: Activity, favourite: bool) -> anyhow::Result<()> {
    let client = client().await?;
    activities::set_favourite(&client, &activity, favourite).await
}

/// Downloads the full activity list (read-only) and returns the updated cache.
pub async fn reload_activities() -> anyhow::Result<Vec<Activity>> {
    let client = client().await?;
    activities::refresh(&client, chrono::Local::now().date_naive()).await
}
