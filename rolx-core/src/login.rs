use crate::config::AppConfig;
use crate::http;
use crate::oauth2::device_flow;
use crate::oauth2::native_app_flow;
use crate::rolx::sign_in;
use crate::session::Session;
use anyhow::anyhow;

// Uses Google OAuth, see:
// - https://developers.google.com/identity/oauth2/native-app
// - https://accounts.google.com/.well-known/oauth-authorization-server
const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_DEVICE_AUTH_URL: &str = "https://oauth2.googleapis.com/device/code";

#[derive(Debug)]
pub enum AuthFlow {
    NativeApp,
    Device,
}

/// Something the user has to do to complete the login, shown by the frontend.
#[derive(Debug)]
pub enum UserAction<'a> {
    /// Opening the browser failed, so the user has to open `url` manually.
    OpenUrl { url: &'a str },
    /// Device flow: the user has to visit `url` and enter `code` there.
    EnterCode { url: &'a str, code: &'a str },
}

/// Logs in and saves the session. Calls `on_user_action` if the user has to do more than log in in the browser
/// that is opened automatically.
#[tracing::instrument(name = "login", skip_all)]
pub async fn run(
    app_cfg: &AppConfig,
    flow: AuthFlow,
    on_user_action: impl FnOnce(UserAction),
) -> anyhow::Result<Session> {
    tracing::info!(
        url = app_cfg.server_url,
        oauth_flow = ?flow,
        "Logging in to"
    );
    let http = http::client_builder()
        .redirect(reqwest::redirect::Policy::none())
        .https_only(true)
        .tls_version_min(reqwest::tls::Version::TLS_1_2)
        .build()?;

    let session = Session::load().ok();
    let id_token = match flow {
        AuthFlow::NativeApp => {
            let credentials = app_cfg.google_desktop_client.as_ref().ok_or_else(|| {
                anyhow!("`google_desktop_client` config missing — add it to the config")
            })?;
            let cfg = native_app_flow::Cfg {
                authorization_endpoint: GOOGLE_AUTH_URL.into(),
                token_endpoint: GOOGLE_TOKEN_URL.into(),
                client_id: credentials.id.clone(),
                client_secret: Some(credentials.secret.clone()),
                user_email: session.map(|s| s.user_email),
            };
            let resp = native_app_flow::run(
                &http,
                &mut rand::rng(),
                &cfg,
                "openid email profile",
                |url| on_user_action(UserAction::OpenUrl { url }),
            )
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "native app flow failed");
                e
            })?;
            resp.id_token
                .ok_or_else(|| anyhow!("id_token missing in access token response"))?
        }
        AuthFlow::Device => {
            let credentials = app_cfg.google_device_client.as_ref().ok_or_else(|| {
                anyhow!("`google_device_client` config missing — add it the config")
            })?;
            let cfg = device_flow::Cfg {
                device_authorization_endpoint: GOOGLE_DEVICE_AUTH_URL.into(),
                token_endpoint: GOOGLE_TOKEN_URL.into(),
                client_id: credentials.id.clone(),
                client_secret: Some(credentials.secret.clone()),
            };
            let resp = device_flow::run(&http, &cfg, "openid email profile", |url, code| {
                on_user_action(UserAction::EnterCode { url, code })
            })
            .await?;
            resp.id_token
                .ok_or_else(|| anyhow!("id_token missing in device flow token response"))?
        }
    };
    let session = sign_in::sign_in(&http, &app_cfg.server_url, &id_token).await?;
    session.save()?;

    tracing::info!(
        name = format!("{} {}", session.user_first_name, session.user_last_name),
        email = session.user_email,
        "Logged in as"
    );
    Ok(session)
}
