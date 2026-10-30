use crate::config::AppConfig;
use crate::rolx::sign_in;
use crate::session::{LoadError, Session};
use anyhow::bail;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub struct RolxClient {
    pub http: reqwest::Client,
    pub server_url: String,
    pub session: Session,
}

impl RolxClient {
    fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.server_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    /// Authenticated `GET` of `path` (relative to the server URL), parsing the JSON response.
    pub async fn get_json<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let url = self.url(path);
        tracing::debug!(method = "GET", url, "HTTP request");
        let resp = self
            .http
            .get(&url)
            .bearer_auth(self.session.bearer_token.as_str())
            .send()
            .await?;
        let status = resp.status();
        let body = resp.text().await?;
        // TODO redact body?
        tracing::debug!(status = status.as_u16(), body, "HTTP response");
        if !status.is_success() {
            bail!("HTTP {status}");
        }
        Ok(serde_json::from_str(&body)?)
    }

    /// Authenticated `PUT` of a JSON body. The error includes the server's validation message.
    pub async fn put_json(&self, path: &str, body: &impl Serialize) -> anyhow::Result<()> {
        self.send_json(reqwest::Method::PUT, path, body).await
    }

    /// Authenticated `DELETE` with a JSON body (the favourites endpoint expects one).
    pub async fn delete_json(&self, path: &str, body: &impl Serialize) -> anyhow::Result<()> {
        self.send_json(reqwest::Method::DELETE, path, body).await
    }

    async fn send_json(
        &self,
        method: reqwest::Method,
        path: &str,
        body: &impl Serialize,
    ) -> anyhow::Result<()> {
        let url = self.url(path);
        tracing::debug!(method = %method, url, "HTTP request");
        let resp = self
            .http
            .request(method, &url)
            .bearer_auth(self.session.bearer_token.as_str())
            .json(body)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            tracing::debug!(status = status.as_u16(), body = text, "HTTP response");
            if text.trim().is_empty() {
                bail!("HTTP {status}");
            }
            bail!("HTTP {status}: {}", text.trim());
        }
        Ok(())
    }

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
