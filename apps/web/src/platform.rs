use std::rc::Rc;

use rolx_core::session::Session;
use rolx_core::store::{SessionStore, StoreError};
use rolx_ui::{LoginFuture, LoginMethod, LoginPrompt, Platform};
use serde::Deserialize;

const SESSION_KEY: &str = "rolxapp.session";

/// The browser's local storage.
#[derive(Default)]
pub struct LocalStorageStore;

fn local_storage() -> Result<web_sys::Storage, StoreError> {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .ok_or_else(|| StoreError::Backend("the browser has no local storage".to_string()))
}

fn storage_failed(_: wasm_bindgen::JsValue) -> StoreError {
    StoreError::Backend("the browser's local storage failed".to_string())
}

impl SessionStore for LocalStorageStore {
    fn load(&self) -> Result<Option<Session>, StoreError> {
        let text = local_storage()?
            .get_item(SESSION_KEY)
            .map_err(storage_failed)?;
        Ok(text.map(|text| serde_json::from_str(&text)).transpose()?)
    }

    fn save(&self, session: &Session) -> Result<(), StoreError> {
        local_storage()?
            .set_item(SESSION_KEY, &serde_json::to_string(session)?)
            .map_err(storage_failed)
    }

    fn delete(&self) -> Result<(), StoreError> {
        local_storage()?
            .remove_item(SESSION_KEY)
            .map_err(storage_failed)
    }
}

/// What the host tells the app in `/config.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WebConfig {
    pub google_client_id: Option<String>,
}

/// Fetches `/config.json`. Under a bare `dx serve` there is none, which simply means the defaults.
pub async fn load_config() -> WebConfig {
    let origin = origin().unwrap_or_default();
    let response = reqwest::get(format!("{origin}/config.json")).await;
    match response {
        Ok(response) if response.status().is_success() => response.json().await.unwrap_or_default(),
        _ => WebConfig::default(),
    }
}

fn origin() -> Option<String> {
    web_sys::window()?.location().origin().ok()
}

/// The API lives on the page's own origin: the host proxies it, so there are no CORS rules to meet
/// and no second address to configure.
#[derive(Default)]
pub struct WebPlatform {
    store: LocalStorageStore,
}

impl Platform for WebPlatform {
    fn session_store(&self) -> &dyn SessionStore {
        &self.store
    }

    fn server_url(&self) -> Option<String> {
        origin()
    }

    fn config_location(&self) -> Option<String> {
        None
    }

    fn config_error(&self) -> Option<String> {
        None
    }

    fn login_methods(&self) -> Vec<LoginMethod> {
        Vec::new()
    }

    fn login(&self, _method: LoginMethod, _on_prompt: Rc<dyn Fn(LoginPrompt)>) -> LoginFuture {
        Box::pin(async { Err("Sign-in is not available yet.".to_string()) })
    }

    fn http_client(&self) -> reqwest::Client {
        reqwest::Client::new()
    }

    fn open_external(&self, url: &str) -> bool {
        web_sys::window()
            .and_then(|window| {
                window
                    .open_with_url_and_target(url, "_blank")
                    .ok()
                    .flatten()
            })
            .is_some()
    }

    /// The web login shows nothing to copy, and the browser's clipboard would answer too late for
    /// this synchronous call. The UI then asks to copy by hand.
    fn copy_text(&self, _text: &str) -> bool {
        false
    }
}
