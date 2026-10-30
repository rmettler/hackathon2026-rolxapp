use std::cell::RefCell;
use std::rc::Rc;

use rolx_core::session::Session;
use rolx_core::store::{SessionStore, StoreError};
use rolx_ui::{LoginFuture, LoginMethod, LoginPrompt, Platform};

/// Stub until M14 and M15: keeps the session in memory, knows no server and cannot sign in.
#[derive(Default)]
pub struct AndroidPlatform {
    session: RefCell<Option<Session>>,
}

impl SessionStore for AndroidPlatform {
    fn load(&self) -> Result<Option<Session>, StoreError> {
        Ok(self.session.borrow().clone())
    }

    fn save(&self, session: &Session) -> Result<(), StoreError> {
        *self.session.borrow_mut() = Some(session.clone());
        Ok(())
    }

    fn delete(&self) -> Result<(), StoreError> {
        *self.session.borrow_mut() = None;
        Ok(())
    }
}

impl Platform for AndroidPlatform {
    fn session_store(&self) -> &dyn SessionStore {
        self
    }

    fn server_url(&self) -> Option<String> {
        None
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
        reqwest::Client::new()
    }

    fn open_external(&self, _url: &str) -> bool {
        false
    }

    fn copy_text(&self, _text: &str) -> bool {
        false
    }
}
