use anyhow::bail;
use rolx_core::api::sign_in;
use rolx_core::config::AppConfig;
use rolx_core::session::Session;
use rolx_core::store::{FileSessionStore, SessionStore};

pub struct RolxClient {
    pub http: reqwest::Client,
    pub server_url: String,
    pub session: Session,
}

impl RolxClient {
    pub async fn new(config: &AppConfig) -> anyhow::Result<Self> {
        let http = reqwest::Client::new();
        let store = FileSessionStore::default_location()?;
        let mut session = match store.load() {
            Ok(Some(session)) => session,
            Ok(None) => bail!("no session found — log in first"),
            Err(e) => bail!("failed to load session: {}\n — try log in again", e),
        };

        if session.is_expired() {
            bail!("session is expired — log in again");
        }

        if session.is_near_expiry() {
            match sign_in::extend(&http, &config.server_url, &session.bearer_token).await {
                Ok(refreshed) => {
                    store.save(&refreshed)?;
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
