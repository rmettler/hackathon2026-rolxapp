use crate::config::AppConfig;
use crate::rolx::sign_in;
use crate::session::{LoadError, Session};
use anyhow::bail;
use reqwest::{Method, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Why a request failed, so callers can tell a missing connection from a refusal.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("no connection: {0}")]
    Network(String),
    #[error("server unavailable (HTTP {0})")]
    Unavailable(u16),
    #[error("not authorized — log in again")]
    Unauthorized,
    #[error("{message}")]
    Rejected { status: u16, message: String },
}

/// Whether `error` means the server could not be reached, so retrying later makes sense.
pub fn is_offline(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<ApiError>(),
        Some(ApiError::Network(_) | ApiError::Unavailable(_))
    )
}

/// Whether `error` means the login is no longer valid.
pub fn is_unauthorized(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<ApiError>(),
        Some(ApiError::Unauthorized)
    )
}

/// Whether the server refused the request itself.
pub fn is_rejected(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<ApiError>(),
        Some(ApiError::Rejected { .. })
    )
}

/// The server's validation messages, or the status when there are none.
fn rejection_message(status: StatusCode, body: &str) -> String {
    let fallback = format!("HTTP {status}");
    let Ok(json) = serde_json::from_str::<serde_json::Value>(body) else {
        return fallback;
    };
    let mut messages = Vec::new();
    if let Some(errors) = json.get("errors").and_then(|e| e.as_object()) {
        for value in errors.values() {
            match value {
                serde_json::Value::Array(items) => {
                    messages.extend(items.iter().filter_map(|i| i.as_str().map(str::to_string)));
                }
                serde_json::Value::String(text) => messages.push(text.clone()),
                _ => {}
            }
        }
    }
    if messages.is_empty() {
        return json
            .get("title")
            .and_then(|t| t.as_str())
            .map_or(fallback, str::to_string);
    }
    messages.join("; ")
}

/// Whether the saved login could be used as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Valid,
    Expired,
}

pub struct RolxClient {
    pub http: reqwest::Client,
    pub server_url: String,
    pub session: Session,
}

impl RolxClient {
    /// Like [`RolxClient::new`], but works without a connection: an expired or unextendable
    /// login still gives a client, which then fails its requests with [`ApiError`]s.
    pub async fn connect(config: &AppConfig) -> anyhow::Result<(Self, SessionStatus)> {
        // Bounded, so a dead connection fails instead of hanging the app.
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(20))
            .build()?;
        let mut session = match Session::load() {
            Ok(session) => session,
            Err(LoadError::NoSession) => bail!("no session found — log in first"),
            Err(e) => bail!("failed to load session: {}\n — try log in again", e),
        };

        let mut status = SessionStatus::Valid;
        if session.is_expired() {
            status = SessionStatus::Expired;
        } else if session.is_near_expiry() {
            match sign_in::extend(&http, &config.server_url, &session.bearer_token).await {
                Ok(refreshed) => {
                    refreshed.save()?;
                    session = refreshed;
                }
                Err(sign_in::SignInExtensionError::SessionExpired) => {
                    status = SessionStatus::Expired;
                }
                Err(e) => tracing::warn!(error = %e, "could not extend the session, continuing"),
            }
        }

        let client = Self {
            http,
            server_url: config.server_url.clone(),
            session,
        };
        Ok((client, status))
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

        let resp = req
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        tracing::debug!(status = status.as_u16(), body = text, "HTTP response");

        match status {
            s if s.is_success() => Ok(text),
            StatusCode::UNAUTHORIZED => Err(ApiError::Unauthorized.into()),
            s if s.is_server_error() => Err(ApiError::Unavailable(s.as_u16()).into()),
            StatusCode::FORBIDDEN => Err(ApiError::Rejected {
                status: 403,
                message: "forbidden — not allowed for this user or date".into(),
            }
            .into()),
            StatusCode::NOT_FOUND => Err(ApiError::Rejected {
                status: 404,
                message: "not found".into(),
            }
            .into()),
            s => Err(ApiError::Rejected {
                status: s.as_u16(),
                message: rejection_message(s, &text),
            }
            .into()),
        }
    }
}
