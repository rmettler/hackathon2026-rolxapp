//! Google sign-in for native apps (tokio): the loopback flow with PKCE and the device flow. The UI
//! of a flow is left to the app, through [`AuthPrompt`].

mod oauth2;

use std::fmt;

use oauth2::{device_flow, native_app_flow};

const SCOPE: &str = "openid email profile";

// Uses Google OAuth, see:
// - https://developers.google.com/identity/oauth2/native-app
// - https://accounts.google.com/.well-known/oauth-authorization-server
const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_DEVICE_AUTH_URL: &str = "https://oauth2.googleapis.com/device/code";

/// What a login flow needs from the app's user interface.
pub trait AuthPrompt {
    /// Opens `url` in the user's browser. Returns whether that worked.
    fn open_url(&self, url: &str) -> bool;

    /// The browser could not be opened: asks the user to open `url` by hand.
    fn show_manual_url(&self, url: &str);

    /// Asks the user to visit `uri` and enter `code`.
    fn show_device_code(&self, uri: &str, code: &str);
}

#[derive(Debug, Clone)]
pub enum Flow {
    /// Loopback redirect with PKCE. A known email may simplify the browser step.
    NativeApp { user_email: Option<String> },
    /// Device authorization, for clients that cannot receive a redirect.
    Device,
}

#[derive(Clone)]
pub struct Credentials {
    pub id: String,
    pub secret: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    pub authorization: String,
    pub token: String,
    pub device_authorization: String,
}

impl Endpoints {
    /// Google's endpoints. Debug builds can point them at a mock with `ROLX_GOOGLE_BASE_URL`;
    /// release builds never read it, because it would redirect tokens.
    pub fn google() -> Self {
        Self::resolve(base_url_override())
    }

    fn resolve(base_url: Option<String>) -> Self {
        match base_url {
            Some(base) => Self {
                authorization: format!("{base}/auth"),
                token: format!("{base}/token"),
                device_authorization: format!("{base}/device/code"),
            },
            None => Self {
                authorization: GOOGLE_AUTH_URL.into(),
                token: GOOGLE_TOKEN_URL.into(),
                device_authorization: GOOGLE_DEVICE_AUTH_URL.into(),
            },
        }
    }
}

#[cfg(debug_assertions)]
fn base_url_override() -> Option<String> {
    std::env::var("ROLX_GOOGLE_BASE_URL")
        .ok()
        .filter(|base| !base.is_empty())
}

#[cfg(not(debug_assertions))]
fn base_url_override() -> Option<String> {
    None
}

/// HTTP client for a login: no redirects, TLS 1.2 or newer, HTTPS only. Only with the debug-build
/// override above may it speak plain HTTP, to a mock.
pub fn http_client() -> reqwest::Result<reqwest::Client> {
    build_http_client(base_url_override().is_none())
}

fn build_http_client(https_only: bool) -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .https_only(https_only)
        .tls_version_min(reqwest::tls::Version::TLS_1_2)
        .build()
}

/// The id token Google issued. `Debug` does not show it.
#[derive(Clone)]
pub struct IdToken(String);

impl IdToken {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for IdToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IdToken(***)")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error(transparent)]
    NativeApp(#[from] native_app_flow::Error),

    #[error(transparent)]
    Device(#[from] device_flow::Error),

    #[error("id_token missing in token response")]
    IdTokenMissing,
}

/// Runs `flow` to its end and returns the id token.
pub async fn login(
    http: &reqwest::Client,
    flow: Flow,
    credentials: &Credentials,
    endpoints: &Endpoints,
    prompt: &dyn AuthPrompt,
) -> Result<IdToken, LoginError> {
    let id_token = match flow {
        Flow::NativeApp { user_email } => {
            let cfg = native_app_flow::Cfg {
                authorization_endpoint: endpoints.authorization.clone(),
                token_endpoint: endpoints.token.clone(),
                client_id: credentials.id.clone(),
                client_secret: credentials.secret.clone(),
                user_email,
            };
            native_app_flow::run(http, &mut rand::rng(), &cfg, SCOPE, prompt)
                .await?
                .id_token
        }
        Flow::Device => {
            let cfg = device_flow::Cfg {
                device_authorization_endpoint: endpoints.device_authorization.clone(),
                token_endpoint: endpoints.token.clone(),
                client_id: credentials.id.clone(),
                client_secret: credentials.secret.clone(),
            };
            device_flow::run(http, &cfg, SCOPE, prompt).await?.id_token
        }
    };
    id_token.map(IdToken).ok_or(LoginError::IdTokenMissing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_default_to_google_over_https() {
        let endpoints = Endpoints::resolve(None);
        assert_eq!(endpoints.authorization, GOOGLE_AUTH_URL);
        assert_eq!(endpoints.token, GOOGLE_TOKEN_URL);
        assert_eq!(endpoints.device_authorization, GOOGLE_DEVICE_AUTH_URL);
    }

    #[test]
    fn endpoints_follow_a_base_url() {
        let endpoints = Endpoints::resolve(Some("http://127.0.0.1:1/google".into()));
        assert_eq!(endpoints.authorization, "http://127.0.0.1:1/google/auth");
        assert_eq!(endpoints.token, "http://127.0.0.1:1/google/token");
        assert_eq!(
            endpoints.device_authorization,
            "http://127.0.0.1:1/google/device/code"
        );
    }

    #[tokio::test]
    async fn https_only_client_refuses_plain_http_before_connecting() {
        let client = build_http_client(true).unwrap();
        let error = client.get("http://127.0.0.1:1").send().await.unwrap_err();
        assert!(error.is_builder(), "was refused, not a failed connection");
    }

    #[test]
    fn id_token_debug_hides_the_value() {
        let token = IdToken("fake-value".into());
        assert!(!format!("{token:?}").contains("fake-value"));
    }
}
