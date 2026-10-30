use std::rc::Rc;
use std::time::Duration;

use rolx_auth::{AuthPrompt, Credentials, Endpoints, Flow};
use rolx_core::api::sign_in;
use rolx_core::config::{self, AppConfig, ClientCredentials};
use rolx_core::session::Session;
use rolx_core::store::{FileSessionStore, SessionStore, StoreError};
use rolx_ui::{LoginFuture, LoginMethod, LoginPrompt, Platform};

/// `config.toml` and `session.json` in the same places as the CLI uses, with Google login.
pub struct DesktopPlatform {
    store: Store,
    config: Result<AppConfig, String>,
    http: reqwest::Client,
}

/// The session store, if the cache directory could be determined.
enum Store {
    File(FileSessionStore),
    Unavailable,
}

impl SessionStore for Store {
    fn load(&self) -> Result<Option<Session>, StoreError> {
        match self {
            Store::File(store) => store.load(),
            Store::Unavailable => Err(StoreError::CacheDirNotFound),
        }
    }

    fn save(&self, session: &Session) -> Result<(), StoreError> {
        match self {
            Store::File(store) => store.save(session),
            Store::Unavailable => Err(StoreError::CacheDirNotFound),
        }
    }

    fn delete(&self) -> Result<(), StoreError> {
        match self {
            Store::File(store) => store.delete(),
            Store::Unavailable => Err(StoreError::CacheDirNotFound),
        }
    }
}

impl DesktopPlatform {
    pub fn load() -> Self {
        Self {
            store: FileSessionStore::default_location().map_or(Store::Unavailable, Store::File),
            config: AppConfig::load().map_err(|e| e.to_string()),
            // A hanging server must end in a retryable error, not in loading forever.
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    fn client(&self, method: LoginMethod) -> Result<&ClientCredentials, String> {
        let config = self.config.as_ref().map_err(Clone::clone)?;
        let (client, key) = match method {
            LoginMethod::Browser => (&config.google_desktop_client, "google_desktop_client"),
            LoginMethod::Device => (&config.google_device_client, "google_device_client"),
        };
        client
            .as_ref()
            .ok_or_else(|| format!("`{key}` is missing in the config file"))
    }
}

/// Bridges a login flow to the UI's prompts.
struct UiPrompt {
    on_prompt: Rc<dyn Fn(LoginPrompt)>,
}

impl AuthPrompt for UiPrompt {
    fn open_url(&self, url: &str) -> bool {
        open::that_detached(url).is_ok()
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

impl Platform for DesktopPlatform {
    fn session_store(&self) -> &dyn SessionStore {
        &self.store
    }

    fn server_url(&self) -> Option<String> {
        self.config.as_ref().ok().map(|c| c.server_url.clone())
    }

    fn config_location(&self) -> Option<String> {
        config::config_path().ok().map(|p| p.display().to_string())
    }

    fn config_error(&self) -> Option<String> {
        self.config.as_ref().err().cloned()
    }

    fn login_methods(&self) -> Vec<LoginMethod> {
        vec![LoginMethod::Browser, LoginMethod::Device]
    }

    fn login(&self, method: LoginMethod, on_prompt: Rc<dyn Fn(LoginPrompt)>) -> LoginFuture {
        let credentials = self.client(method).map(|client| Credentials {
            id: client.id.clone(),
            secret: Some(client.secret.clone()),
        });
        let server_url = self.server_url();
        let user_email = self
            .store
            .load()
            .ok()
            .flatten()
            .map(|session| session.user_email);

        Box::pin(async move {
            let credentials = credentials?;
            let server_url = server_url.ok_or("the server is not configured")?;
            let flow = match method {
                LoginMethod::Browser => Flow::NativeApp { user_email },
                LoginMethod::Device => Flow::Device,
            };
            let http = rolx_auth::http_client().map_err(|e| e.to_string())?;
            let id_token = rolx_auth::login(
                &http,
                flow,
                &credentials,
                &Endpoints::google(),
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
        open::that_detached(url).is_ok()
    }

    fn copy_text(&self, text: &str) -> bool {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_text(text))
            .is_ok()
    }
}
