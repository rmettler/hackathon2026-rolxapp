use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::logging::redact_json;
use crate::session::{BearerToken, Session};

#[derive(Debug, thiserror::Error)]
pub enum SignInError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("server did not authorize the ID token — is your account confirmed?")]
    Unauthorized,

    #[error("server rejected sign-in with HTTP {0}")]
    Rejected(u16),
}

pub async fn sign_in(
    client: &reqwest::Client,
    server_url: &str,
    id_token: &str,
) -> std::result::Result<Session, SignInError> {
    let resp = {
        let url = format!("{server_url}/api/v1/SignIn");
        let body = serde_json::json!({ "googleIdToken": id_token });
        client.post(&url).json(&body).send().await?
    };

    let status = resp.status();
    let body = serde_json::from_str::<serde_json::Value>(resp.text().await?.as_ref())?;
    if !status.is_success() {
        tracing::warn!(
            status = status.as_u16(),
            body = redact_json(body.clone(), &["bearerToken", "user"]).to_string(),
            "HTTP response"
        );
        return Err(if status == reqwest::StatusCode::UNAUTHORIZED {
            SignInError::Unauthorized
        } else {
            SignInError::Rejected(status.as_u16())
        });
    }

    Ok(serde_json::from_value::<ApprovalResponse>(body)?.into())
}

#[derive(Debug, thiserror::Error)]
pub enum SignInExtensionError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("session expired")]
    SessionExpired,

    #[error("server rejected sign-in extension with HTTP {0}")]
    Rejected(u16),
}

pub async fn extend(
    client: &reqwest::Client,
    server_url: &str,
    bearer_token: &BearerToken,
) -> std::result::Result<Session, SignInExtensionError> {
    let url = format!("{server_url}/api/v1/SignIn/extend");
    tracing::debug!(method = "GET", url, "HTTP request");

    let resp = client
        .get(&url)
        .bearer_auth(bearer_token.as_str())
        .send()
        .await?;
    let status = resp.status();
    let body = serde_json::from_str::<serde_json::Value>(&resp.text().await?)?;
    tracing::debug!(
        status = status.as_u16(),
        body = redact_json(body.clone(), &["bearerToken", "googleIdToken"]).to_string(),
        "HTTP response"
    );

    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(SignInExtensionError::SessionExpired);
    }
    if !status.is_success() {
        return Err(SignInExtensionError::Rejected(status.as_u16()));
    }

    Ok(serde_json::from_value::<ApprovalResponse>(body)?.into())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserResource {
    id: String,
    email: String,
    first_name: String,
    last_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApprovalResponse {
    user: UserResource,
    bearer_token: String,
    expires: DateTime<Utc>,
}

impl From<ApprovalResponse> for Session {
    fn from(a: ApprovalResponse) -> Self {
        Session {
            bearer_token: BearerToken::from_string(a.bearer_token),
            expires: a.expires,
            user_id: a.user.id,
            user_email: a.user.email,
            user_first_name: a.user.first_name,
            user_last_name: a.user.last_name,
        }
    }
}
