use crate::config::AppConfig;
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

#[tracing::instrument(name = "login", skip_all)]
pub async fn run(app_cfg: &AppConfig, flow: AuthFlow) -> anyhow::Result<()> {
    tracing::info!(
        url = app_cfg.server_url,
        oauth_flow = ?flow,
        "Logging in to"
    );
    let http = reqwest::Client::builder()
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
            let resp = native_app_flow::run(&http, &mut rand::rng(), &cfg, "openid email profile")
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
            let resp = device_flow::run(&http, &cfg, "openid email profile").await?;
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
    Ok(())
}
