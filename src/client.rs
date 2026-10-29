use crate::config::AppConfig;
use crate::rolx::sign_in;
use crate::session::{LoadError, Session};
use anyhow::bail;

pub struct RolxClient {
    pub http: reqwest::Client,
    pub server_url: String,
    pub session: Session,
}

impl RolxClient {
    pub async fn new(config: &AppConfig) -> anyhow::Result<Self> {
        let http = reqwest::Client::new();
        let mut session = match Session::load() {
            Ok(session) => session,
            Err(LoadError::NoSession) => bail!("no session found — log in first"),
            Err(e) => bail!("failed to load session: {}\n — try log in again", e),
        };

        if session.is_expired() {
            bail!("session is expired — log in again");
        }

        if session.is_near_expiry() {
            match sign_in::extend(&http, &config.server_url, &session.bearer_token).await {
                Ok(refreshed) => {
                    refreshed.save()?;
                    session = refreshed;
                }
                Err(sign_in::SignInExtensionError::SessionExpired) => {
                    bail!("session is expired — log in again")
                }
                Err(e) => bail!("failed to extend session: {}\n — try log in again", e),
            }
        }

        Ok(Self {
            http,
            server_url: config.server_url.clone(),
            session,
        })
    }
}
