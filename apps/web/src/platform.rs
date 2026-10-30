use std::pin::pin;
use std::rc::Rc;

use futures_util::future::{Either, select};

use rolx_core::api::sign_in;
use rolx_core::session::Session;
use rolx_core::store::{SessionStore, StoreError};
use rolx_ui::{LoginFuture, LoginMethod, LoginPrompt, LoginResultHandler, Platform};
use serde::Deserialize;

use crate::google;

const SESSION_KEY: &str = "rolxapp.session";
const CONFIG_TIMEOUT_MS: u32 = 10_000;

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

/// Fetches `/config.json`. Under a bare `dx serve` there is none, which simply means the defaults;
/// so does a host that does not answer in time, as the page waits for this.
pub async fn load_config() -> WebConfig {
    let origin = origin().unwrap_or_default();
    let load = async {
        match reqwest::get(format!("{origin}/config.json")).await {
            Ok(response) if response.status().is_success() => {
                response.json().await.unwrap_or_default()
            }
            _ => WebConfig::default(),
        }
    };
    let timeout = gloo_timers::future::TimeoutFuture::new(CONFIG_TIMEOUT_MS);
    match select(pin!(load), timeout).await {
        Either::Left((config, _)) => config,
        Either::Right(_) => WebConfig::default(),
    }
}

fn origin() -> Option<String> {
    web_sys::window()?.location().origin().ok()
}

/// The API lives on the page's own origin: the host proxies it, so there are no CORS rules to meet
/// and no second address to configure.
pub struct WebPlatform {
    store: LocalStorageStore,
    config: WebConfig,
}

impl WebPlatform {
    pub fn new(config: WebConfig) -> Self {
        Self {
            store: LocalStorageStore,
            config,
        }
    }
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
        match self.config.google_client_id {
            Some(_) => vec![LoginMethod::GoogleButton],
            None => Vec::new(),
        }
    }

    fn login(&self, _method: LoginMethod, _on_prompt: Rc<dyn Fn(LoginPrompt)>) -> LoginFuture {
        Box::pin(async { Err("This app signs in with Google's button.".to_string()) })
    }

    fn mount_login_button(&self, element_id: &str, on_result: LoginResultHandler) {
        let Some(client_id) = self.config.google_client_id.as_deref() else {
            return;
        };
        let server_url = origin().unwrap_or_default();
        let http = self.http_client();
        let on_error = on_result.clone();
        let on_credential = Rc::new(move |id_token: String| {
            let (http, server_url, on_result) =
                (http.clone(), server_url.clone(), on_result.clone());
            wasm_bindgen_futures::spawn_local(async move {
                let result = sign_in::sign_in(&http, &server_url, &id_token)
                    .await
                    .map_err(|e| e.to_string());
                on_result(result);
            });
        });
        google::mount_button(
            element_id,
            client_id,
            on_credential,
            Rc::new(move |error| on_error(Err(error))),
        );
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
