use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::redact::redact_json;
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
    let body = resp.text().await?;
    if !status.is_success() {
        tracing::warn!(
            status = status.as_u16(),
            body = loggable_body(&body),
            "HTTP response"
        );
        return Err(if status == reqwest::StatusCode::UNAUTHORIZED {
            SignInError::Unauthorized
        } else {
            SignInError::Rejected(status.as_u16())
        });
    }

    Ok(serde_json::from_str::<ApprovalResponse>(&body)?.into())
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
    let body = resp.text().await?;
    tracing::debug!(
        status = status.as_u16(),
        body = loggable_body(&body),
        "HTTP response"
    );

    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(SignInExtensionError::SessionExpired);
    }
    if !status.is_success() {
        return Err(SignInExtensionError::Rejected(status.as_u16()));
    }

    Ok(serde_json::from_str::<ApprovalResponse>(&body)?.into())
}

/// The body with secrets and personal data redacted. Error bodies are often empty or not JSON, and
/// are then left out rather than logged unredacted.
fn loggable_body(body: &str) -> String {
    match serde_json::from_str(body) {
        Ok(value) => redact_json(value, &["bearerToken", "googleIdToken", "user"]).to_string(),
        Err(_) => format!("<{} bytes, not JSON>", body.len()),
    }
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

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    /// Shape of `Auth/WebApi/Resource/Approval.cs` with its `Users/WebApi/Resource/User.cs`.
    const APPROVAL: &str = r#"{
        "user": {
            "id": "00000000-0000-0000-0000-000000000001",
            "firstName": "Test",
            "lastName": "User",
            "email": "test@example.invalid",
            "avatarUrl": "",
            "role": 1,
            "entryDate": null,
            "leavingDate": null,
            "isConfirmed": true,
            "partTimeSettings": [],
            "balanceCorrections": [],
            "vacationDaysSettings": [],
            "targetBillabilitySettings": [],
            "positionSettings": []
        },
        "bearerToken": "fake-token",
        "expires": "2030-01-02T03:04:05Z"
    }"#;

    #[test]
    fn approval_becomes_a_session() {
        let session: Session = serde_json::from_str::<ApprovalResponse>(APPROVAL)
            .unwrap()
            .into();

        assert_eq!(session.bearer_token.as_str(), "fake-token");
        assert_eq!(
            session.expires,
            Utc.with_ymd_and_hms(2030, 1, 2, 3, 4, 5).unwrap()
        );
        assert_eq!(session.user_id, "00000000-0000-0000-0000-000000000001");
        assert_eq!(session.user_email, "test@example.invalid");
        assert_eq!(session.user_first_name, "Test");
        assert_eq!(session.user_last_name, "User");
    }

    #[cfg(not(feature = "uncensored_logs"))]
    #[test]
    fn logged_body_hides_token_and_user() {
        let logged = loggable_body(APPROVAL);
        assert!(!logged.contains("fake-token"));
        assert!(!logged.contains("test@example.invalid"));
    }

    #[test]
    fn logged_body_never_contains_text_that_is_not_json() {
        assert_eq!(loggable_body("secret text"), "<11 bytes, not JSON>");
        assert_eq!(loggable_body(""), "<0 bytes, not JSON>");
    }
}
