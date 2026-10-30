use crate::oauth2::pkce;
use serde::Deserialize;
use std::fmt::Display;

pub async fn request_access_token(
    http_client: &reqwest::Client,
    token_endpoint: &str,
    client_id: &str,
    client_secret: Option<&str>,
    redirect_uri: &str,
    code_verifier: pkce::CodeVerifier,
    authorization_code: &str,
) -> Result<Response, Error> {
    let mut params = vec![
        ("client_id", client_id),
        ("code", authorization_code),
        ("code_verifier", code_verifier.as_str()),
        ("grant_type", "authorization_code"),
        ("redirect_uri", redirect_uri),
    ];
    params.extend(client_secret.into_iter().map(|s| ("client_secret", s)));

    #[cfg(not(feature = "uncensored_logs"))]
    tracing::debug!(
        method = "POST",
        url = token_endpoint,
        "request access token"
    );
    #[cfg(feature = "uncensored_logs")]
    tracing::debug!(
        method = "POST",
        url = token_endpoint,
        body = ?params,
        "request access token"
    );

    let resp = http_client
        .post(token_endpoint)
        .form(&params)
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;

    #[cfg(not(feature = "uncensored_logs"))]
    tracing::debug!(status = status.as_u16(), "access token response");
    #[cfg(feature = "uncensored_logs")]
    tracing::debug!(
        status = status.as_u16(),
        body = text,
        "access token response"
    );

    if status.is_success() {
        Ok(serde_json::from_str(&text)?)
    } else if status.is_client_error() {
        Err(Error::Response(serde_json::from_str(&text)?))
    } else {
        Err(Error::InvalidHttpStatus(status))
    }
}

#[derive(Debug, Deserialize)]
#[allow(unused)]
pub struct Response {
    pub access_token: String,
    /// in seconds
    pub expires_in: Option<u64>,
    pub id_token: Option<String>,
    pub refresh_token: Option<String>,
    /// in seconds
    pub refresh_token_expires_in: Option<u64>,
    pub scope: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("received invalid HTTP status: {0}")]
    InvalidHttpStatus(reqwest::StatusCode),

    #[error("responded error: {0}")]
    Response(ErrorResponse),
}

/// Error sent with the response to the access token request
#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    /// Expected error codes by RFC 6749 are:
    /// invalid_request, invalid_client, invalid_grant, unauthorized_client, unsupported_grant_type, invalid_scope
    error: String,
    error_description: Option<String>,
    error_uri: Option<String>,
}

impl Display for ErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error '{}'", self.error)?;
        if let Some(descr) = &self.error_description {
            write!(f, ", error description '{}'", descr)?;
        }
        if let Some(uri) = &self.error_uri {
            write!(f, ", error URI '{}'", uri)?;
        }
        Ok(())
    }
}
