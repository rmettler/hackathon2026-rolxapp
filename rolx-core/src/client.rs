use crate::config::AppConfig;
use crate::rolx::sign_in;
use crate::session::{LoadError, Session};
use anyhow::bail;
use reqwest::{Method, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;

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

    /// `GET`s `path` (e.g. `api/v1/Balance/2026-01-01`) and deserializes the JSON response.
    pub async fn get_json<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let body = self.send(Method::GET, path, None).await?;
        Ok(serde_json::from_str(&body)?)
    }

    /// `PUT`s `body` as JSON to `path`, expecting an empty success response.
    pub async fn put_json(&self, path: &str, body: &impl Serialize) -> anyhow::Result<()> {
        self.send(Method::PUT, path, Some(serde_json::to_value(body)?))
            .await?;
        Ok(())
    }

    /// `DELETE`s `path` with `body` as JSON, expecting an empty success response.
    pub async fn delete_json(&self, path: &str, body: &impl Serialize) -> anyhow::Result<()> {
        self.send(Method::DELETE, path, Some(serde_json::to_value(body)?))
            .await?;
        Ok(())
    }

    async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> anyhow::Result<String> {
        let url = format!("{}/{}", self.server_url, path);
        tracing::debug!(method = %method, url, "HTTP request");
        let mut req = self
            .http
            .request(method, &url)
            .bearer_auth(self.session.bearer_token.as_str());
        if let Some(body) = body {
            req = req.json(&body);
        }

        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        tracing::debug!(status = status.as_u16(), body = text, "HTTP response");

        match status {
            s if s.is_success() => Ok(text),
            StatusCode::UNAUTHORIZED => bail!("not authorized — log in again"),
            StatusCode::FORBIDDEN => bail!("forbidden — not allowed for this user or date"),
            StatusCode::NOT_FOUND => bail!("not found"),
            s => bail!("HTTP {s}"),
        }
    }
}
