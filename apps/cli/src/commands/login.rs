use anyhow::anyhow;
use rolx_auth::{AuthPrompt, Credentials, Endpoints, Flow};
use rolx_core::api::sign_in;
use rolx_core::config::{AppConfig, ClientCredentials};
use rolx_core::store::{FileSessionStore, SessionStore};

#[derive(Debug)]
pub enum AuthFlow {
    NativeApp,
    Device,
}

struct TerminalPrompt;

impl AuthPrompt for TerminalPrompt {
    fn open_url(&self, url: &str) -> bool {
        match open::that(url) {
            Ok(()) => {
                tracing::info!("Opened browser for user authorization…");
                true
            }
            Err(e) => {
                tracing::warn!(error = %e, "Opening browser failed; open URL manually");
                false
            }
        }
    }

    fn show_manual_url(&self, url: &str) {
        println!("Opening browser failed!\n\nOpen this URL in your browser:\n  {url}");
    }

    fn show_device_code(&self, uri: &str, code: &str) {
        println!("To authorize this application, visit:\n  {uri}");
        println!("Then enter the following code: {code}\n");
    }
}

fn credentials(client: &ClientCredentials) -> Credentials {
    Credentials {
        id: client.id.clone(),
        secret: Some(client.secret.clone()),
    }
}

#[tracing::instrument(name = "login", skip_all)]
pub async fn run(app_cfg: &AppConfig, flow: AuthFlow) -> anyhow::Result<()> {
    tracing::info!(
        url = app_cfg.server_url,
        oauth_flow = ?flow,
        "Logging in to"
    );
    let http = rolx_auth::http_client()?;

    let store = FileSessionStore::default_location()?;
    let (flow, client) = match flow {
        AuthFlow::NativeApp => {
            let client = app_cfg.google_desktop_client.as_ref().ok_or_else(|| {
                anyhow!("`google_desktop_client` config missing — add it to the config")
            })?;
            let user_email = store.load().ok().flatten().map(|s| s.user_email);
            (Flow::NativeApp { user_email }, client)
        }
        AuthFlow::Device => {
            let client = app_cfg.google_device_client.as_ref().ok_or_else(|| {
                anyhow!("`google_device_client` config missing — add it the config")
            })?;
            (Flow::Device, client)
        }
    };

    let id_token = rolx_auth::login(
        &http,
        flow,
        &credentials(client),
        &Endpoints::google(),
        &TerminalPrompt,
    )
    .await
    .inspect_err(|e| tracing::error!(error = %e, "login flow failed"))?;

    let session = sign_in::sign_in(&http, &app_cfg.server_url, id_token.as_str()).await?;
    store.save(&session)?;

    tracing::info!(
        name = format!("{} {}", session.user_first_name, session.user_last_name),
        email = session.user_email,
        "Logged in as"
    );
    Ok(())
}
