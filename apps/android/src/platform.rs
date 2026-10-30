use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use rolx_core::store::{FileSessionStore, SessionStore};
use rolx_ui::{LoginFuture, LoginMethod, LoginPrompt, Platform, normalize_server_url};

/// Compiled in by `build.rs`, from the environment or `config.local.toml`.
const BUILD_SERVER_URL: Option<&str> = option_env!("ROLX_SERVER_URL");

/// Device login until M15. The session and a server chosen in the settings live in the app's private
/// data directory. TLS trusts the bundled roots, because the default verifier would need the
/// Android JNI context set up first.
pub struct AndroidPlatform {
    store: FileSessionStore,
    server_file: PathBuf,
    server_url: RefCell<Option<String>>,
    http: reqwest::Client,
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
            http: http_client(),
        }
    }
}

fn http_client() -> reqwest::Client {
    let roots = webpki_root_certs::TLS_SERVER_ROOT_CERTS
        .iter()
        .filter_map(|cert| reqwest::Certificate::from_der(cert.as_ref()).ok());
    reqwest::Client::builder()
        .tls_certs_only(roots)
        // The mock in the emulator speaks plain HTTP, nothing else may.
        .https_only(!cfg!(debug_assertions))
        .timeout(Duration::from_secs(30))
        .build()
        .expect("the HTTP client's configuration is fixed")
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

    fn login(&self, _method: LoginMethod, _on_prompt: Rc<dyn Fn(LoginPrompt)>) -> LoginFuture {
        Box::pin(async { Err("Sign-in is not available yet.".to_string()) })
    }

    fn http_client(&self) -> reqwest::Client {
        self.http.clone()
    }

    fn open_external(&self, _url: &str) -> bool {
        false
    }

    fn copy_text(&self, _text: &str) -> bool {
        false
    }
}
