use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use rolx_auth::{AuthPrompt, Credentials, Endpoints, Flow};
use rolx_core::api::sign_in;
use rolx_core::store::{FileSessionStore, SessionStore};
use rolx_ui::{LoginFuture, LoginMethod, LoginPrompt, Platform, normalize_server_url};

use crate::system;

/// Compiled in by `build.rs`, from the environment or `config.local.toml`.
const BUILD_SERVER_URL: Option<&str> = option_env!("ROLX_SERVER_URL");
const DEVICE_CLIENT_ID: Option<&str> = option_env!("ROLX_GOOGLE_DEVICE_CLIENT_ID");
const DEVICE_CLIENT_SECRET: Option<&str> = option_env!("ROLX_GOOGLE_DEVICE_CLIENT_SECRET");
/// Points the Google endpoints at the mock. Not in release builds, as it would redirect tokens.
#[cfg(debug_assertions)]
const GOOGLE_BASE_URL: Option<&str> = option_env!("ROLX_ANDROID_GOOGLE_BASE_URL");

/// Signs in with the device flow only: never OAuth inside the WebView. The session and a server
/// chosen in the settings live in the app's private data directory. TLS trusts the bundled roots,
/// because the default verifier would need the Android JNI context set up first.
pub struct AndroidPlatform {
    store: FileSessionStore,
    server_file: PathBuf,
    server_url: RefCell<Option<String>>,
    http: reqwest::Client,
    login_http: reqwest::Client,
}

impl AndroidPlatform {
    pub fn load() -> Self {
        let data_dir = dioxus_sdk_storage::data_directory().join("rolx");
        let server_file = data_dir.join("server_url");
        let server_url = fs::read_to_string(&server_file)
            .ok()
            .map(|url| url.trim().to_string())
            .filter(|url| !url.is_empty())
            .or_else(|| BUILD_SERVER_URL.and_then(|url| normalize_server_url(url).ok()));
        Self {
            store: FileSessionStore::new(data_dir.join("session.json")),
            server_file,
            server_url: RefCell::new(server_url),
            http: http_client(reqwest::redirect::Policy::default()),
            login_http: http_client(reqwest::redirect::Policy::none()),
        }
    }
}

fn http_client(redirects: reqwest::redirect::Policy) -> reqwest::Client {
    let roots = webpki_root_certs::TLS_SERVER_ROOT_CERTS
        .iter()
        .filter_map(|cert| reqwest::Certificate::from_der(cert.as_ref()).ok());
    reqwest::Client::builder()
        .tls_certs_only(roots)
        .redirect(redirects)
        // The mock in the emulator speaks plain HTTP, nothing else may.
        .https_only(!cfg!(debug_assertions))
        .timeout(Duration::from_secs(30))
        .build()
        .expect("the HTTP client's configuration is fixed")
}

#[cfg(debug_assertions)]
fn endpoints() -> Endpoints {
    GOOGLE_BASE_URL.map_or_else(Endpoints::google, Endpoints::at_base_url)
}

#[cfg(not(debug_assertions))]
fn endpoints() -> Endpoints {
    Endpoints::google()
}

/// Bridges a login flow to the UI's prompts.
struct UiPrompt {
    on_prompt: Rc<dyn Fn(LoginPrompt)>,
}

impl AuthPrompt for UiPrompt {
    fn open_url(&self, url: &str) -> bool {
        system::open_in_browser(url)
    }

    fn show_manual_url(&self, url: &str) {
        (self.on_prompt)(LoginPrompt::ManualUrl(url.to_string()));
    }

    fn show_device_code(&self, uri: &str, code: &str) {
        (self.on_prompt)(LoginPrompt::DeviceCode {
            uri: uri.to_string(),
            code: code.to_string(),
        });
    }
}

impl Platform for AndroidPlatform {
    fn session_store(&self) -> &dyn SessionStore {
        &self.store
    }

    fn server_url(&self) -> Option<String> {
        self.server_url.borrow().clone()
    }

    fn can_change_server(&self) -> bool {
        true
    }

    fn validate_server_url(&self, url: &str) -> Result<String, String> {
        let url = normalize_server_url(url)?;
        if !cfg!(debug_assertions) && !url.starts_with("https://") {
            return Err("The address must start with https://.".to_string());
        }
        Ok(url)
    }

    fn set_server_url(&self, url: &str) -> Result<(), String> {
        let url = self.validate_server_url(url)?;
        let saved = self
            .server_file
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|()| fs::write(&self.server_file, &url));
        saved.map_err(|_| "The server could not be saved.".to_string())?;
        *self.server_url.borrow_mut() = Some(url);
        Ok(())
    }

    fn config_location(&self) -> Option<String> {
        None
    }

    fn config_error(&self) -> Option<String> {
        None
    }

    fn login_methods(&self) -> Vec<LoginMethod> {
        vec![LoginMethod::Device]
    }

    fn login(&self, _method: LoginMethod, on_prompt: Rc<dyn Fn(LoginPrompt)>) -> LoginFuture {
        let credentials = DEVICE_CLIENT_ID.map(|id| Credentials {
            id: id.to_string(),
            secret: DEVICE_CLIENT_SECRET.map(str::to_string),
        });
        let server_url = self.server_url();
        let http = self.login_http.clone();

        Box::pin(async move {
            let credentials = credentials.ok_or("This build has no Google client configured.")?;
            let server_url = server_url.ok_or("The server is not configured.")?;
            let id_token = rolx_auth::login(
                &http,
                Flow::Device,
                &credentials,
                &endpoints(),
                &UiPrompt { on_prompt },
            )
            .await
            .map_err(|e| e.to_string())?;
            sign_in::sign_in(&http, &server_url, id_token.as_str())
                .await
                .map_err(|e| e.to_string())
        })
    }

    fn http_client(&self) -> reqwest::Client {
        self.http.clone()
    }

    fn open_external(&self, url: &str) -> bool {
        system::open_in_browser(url)
    }

    fn copy_text(&self, text: &str) -> bool {
        system::copy_to_clipboard(text)
    }
}
