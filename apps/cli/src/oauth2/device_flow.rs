//! Implements the RFC 8628 OAuth 2.0 Device Authorization Grant for clients without browser access.

use std::time::Duration;
pub use token::TokenResponse;

pub struct Cfg {
    /// URL of the device authorization endpoint.
    pub device_authorization_endpoint: String,
    /// URL of the token endpoint, polling for the authorization result.
    pub token_endpoint: String,
    /// Client identifier issued to the client.
    pub client_id: String,
    /// Client secret, if the client is registered or requires authentication.
    pub client_secret: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("device authorization failed: {0}")]
    DeviceAuthorization(#[from] auth::Error),

    #[error("access token exchange failed: {0}")]
    TokenExchange(#[from] token::Error),
}

pub async fn run(
    http_client: &reqwest::Client,
    cfg: &Cfg,
    scope: &str,
) -> Result<TokenResponse, Error> {
    let device_resp = auth::request_device_code(http_client, cfg, scope).await?;

    let verification_uri = device_resp
        .verification_uri_complete
        .as_ref()
        .unwrap_or(&device_resp.verification_uri);

    println!(
        "To authorize this application, visit:\n  {}",
        verification_uri
    );
    println!("Then enter the following code: {}\n", device_resp.user_code);

    let resp = token::poll_for_token(
        http_client,
        cfg.token_endpoint.as_str(),
        cfg.client_id.as_str(),
        cfg.client_secret.as_deref(),
        device_resp.device_code.as_str(),
        Duration::from_secs(device_resp.expires_in),
        Duration::from_secs(device_resp.interval.unwrap_or(5)),
    )
    .await?;
    Ok(resp)
}

fn log_safe_form(params: &[(&str, &str)], sensitive_keys: &[&str]) -> String {
    params
        .iter()
        .map(|(key, value)| {
            if sensitive_keys.contains(key) {
                format!("{key}=***")
            } else {
                format!("{key}={value}")
            }
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn log_safe_json(text: &str, sensitive_keys: &[&str]) -> String {
    let mut value = match serde_json::from_str::<serde_json::Value>(text) {
        Ok(value) => value,
        Err(_) => return text.to_string(),
    };

    redact_value(&mut value, sensitive_keys);
    value.to_string()
}

fn redact_value(value: &mut serde_json::Value, sensitive_keys: &[&str]) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if sensitive_keys.contains(&key.as_str()) {
                    *val = serde_json::Value::String("***".into());
                } else {
                    redact_value(val, sensitive_keys);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items.iter_mut() {
                redact_value(item, sensitive_keys);
            }
        }
        _ => {}
    }
}

mod auth {
    use super::Cfg;
    use serde::Deserialize;
    use std::fmt::Display;

    /// Represents the response from the device authorization endpoint.
    #[derive(Debug, Deserialize)]
    pub struct DeviceCodeResponse {
        /// The unique device code for authorizing the device.
        pub device_code: String,

        /// The code the user must enter at the verification URI.
        pub user_code: String,

        /// While RFC 8628 calls it `verification_uri`, Google OAuth provides it as `verification_url`
        #[serde(alias = "verification_url")]
        pub verification_uri: String,

        /// The complete URL for the user to visit including the user code pre-filled.
        /// RFC 8628 recommends using this if available, as it may skip manual entry.
        pub verification_uri_complete: Option<String>,

        /// The lifetime of the device code in seconds (default 180).
        pub expires_in: u64,

        /// The minimum amount of time in seconds that the client
        /// SHOULD wait between polling requests. Default is 5 seconds.
        pub(crate) interval: Option<u64>,
    }

    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error("HTTP request failed: {0}")]
        Http(#[from] reqwest::Error),

        #[error("JSON error: {0}")]
        Json(#[from] serde_json::Error),

        #[error("invalid HTTP status: {0}")]
        InvalidHttpStatus(reqwest::StatusCode),

        #[error("responded error: {0}")]
        Response(ErrorResponse),
    }

    #[derive(Debug, Deserialize)]
    pub struct ErrorResponse {
        /// Expected error codes by RFC 8628 (Device Grant errors):
        /// invalid_request, access_denied, insufficient_scope,
        /// invalid_client, internal_error
        error: String,
        error_description: Option<String>,
        error_uri: Option<String>,
    }

    impl Display for ErrorResponse {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.error)?;
            if let Some(descr) = &self.error_description {
                write!(f, r#", "{}""#, descr)?;
            }
            if let Some(uri) = &self.error_uri {
                write!(f, ", {}", uri)?;
            }
            Ok(())
        }
    }

    pub async fn request_device_code(
        http_client: &reqwest::Client,
        cfg: &Cfg,
        scope: &str,
    ) -> Result<DeviceCodeResponse, Error> {
        let mut body = vec![("client_id", cfg.client_id.as_str()), ("scope", scope)];

        if let Some(ref secret) = cfg.client_secret {
            body.push(("client_secret", secret.as_str()));
        }

        let log_body_str = super::log_safe_form(&body, &["client_id", "client_secret"]);
        tracing::debug!(
            method = "POST",
            url = %cfg.device_authorization_endpoint,
            "HTTP request"
        );
        tracing::debug!(
            method = "POST",
            url = %cfg.device_authorization_endpoint,
            body = %log_body_str,
            "HTTP request body"
        );

        let resp = http_client
            .post(&cfg.device_authorization_endpoint)
            .form(&body)
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        let log_response = super::log_safe_json(
            &text,
            &["device_code", "user_code", "verification_uri_complete"],
        );
        tracing::debug!(status = status.as_u16(), body = %log_response, "HTTP response");

        if status.is_success() {
            Ok(serde_json::from_str(&text)?)
        } else if status.is_client_error() {
            Err(Error::Response(serde_json::from_str(&text)?))
        } else {
            Err(Error::InvalidHttpStatus(status))
        }
    }
}

mod token {
    use serde::Deserialize;
    use std::fmt::Display;
    use std::time::Duration;
    use tokio::time::{sleep, timeout};

    #[derive(Debug, Deserialize)]
    #[allow(unused)]
    pub struct TokenResponse {
        /// The access token issued by the authorization server.
        pub access_token: Option<String>,
        /// Type of the token (typically "Bearer").
        pub token_type: Option<String>,
        /// Lifetime in seconds.
        pub expires_in: Option<u64>,
        /// ID token containing user identity information (OpenID Connect).
        pub id_token: Option<String>,
        /// Refresh token that can be used to obtain new tokens.
        pub refresh_token: Option<String>,
        /// Scopes the token is valid for.
        pub scope: Option<String>,
    }

    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error("HTTP request failed: {0}")]
        Http(#[from] reqwest::Error),

        #[error("JSON error: {0}")]
        Json(#[from] serde_json::Error),

        #[error("responded error: {0}")]
        Response(ErrorResponse),

        #[error("responded invalid error '{0}': {1}")]
        InvalidResponse(String, ErrorResponse),

        #[error("device authorization expired")]
        Expired,
    }

    #[derive(Debug, Deserialize)]
    pub struct ErrorResponse {
        /// Expected error codes by RFC 8628 §3.5:
        /// authorization_pending, slow_down, access_denied, expired_token
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

    pub async fn poll_for_token(
        http_client: &reqwest::Client,
        token_endpoint: &str,
        client_id: &str,
        client_secret: Option<&str>,
        device_code: &str,
        expires_in: Duration,
        mut interval: Duration,
    ) -> Result<TokenResponse, Error> {
        let body = create_request_body(device_code, client_id, client_secret);
        let log_body_str =
            super::log_safe_form(&body, &["client_id", "device_code", "client_secret"]);

        let result = timeout(expires_in, async {
            loop {
                tracing::debug!(method = "POST", url = %token_endpoint, "HTTP request");
                tracing::debug!(
                    method = "POST",
                    url = %token_endpoint,
                    body = %log_body_str,
                    "HTTP request body"
                );

                let resp = http_client.post(token_endpoint).form(&body).send().await?;
                let status = resp.status();
                let text = resp.text().await?;
                let log_response =
                    super::log_safe_json(&text, &["access_token", "id_token", "refresh_token"]);
                tracing::debug!(status = status.as_u16(), body = %log_response, "HTTP response");

                if status.is_success() {
                    return Ok(serde_json::from_str(&text)?);
                } else if status.is_client_error() {
                    let parsed: ErrorResponse = serde_json::from_str(&text)?;
                    match parsed.error.as_str() {
                        "authorization_pending" => {}
                        "slow_down" => {
                            // RFC 8628 §3.5: The authorization request is still pending and polling
                            // should continue, but the interval MUST be increased by 5 seconds for
                            // this and all subsequent requests.
                            interval += Duration::from_secs(5);
                        }
                        "access_denied" => {
                            return Err(Error::Response(parsed));
                        }
                        "expired_token" => {
                            return Err(Error::Expired);
                        }
                        other => {
                            return Err(Error::InvalidResponse(other.into(), parsed));
                        }
                    }
                }

                sleep(interval).await;
            }
        })
        .await;

        result.unwrap_or(Err(Error::Expired))
    }

    // TODO: check and describe, client_secret is not mentioned in RFC 8628, however, Google OAuth seems to use it
    fn create_request_body<'a>(
        device_code: &'a str,
        client_id: &'a str,
        client_secret: Option<&'a str>,
    ) -> Vec<(&'a str, &'a str)> {
        let mut body = vec![
            ("client_id", client_id),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ];
        if let Some(secret) = client_secret {
            body.push(("client_secret", secret));
        }
        body
    }
}
