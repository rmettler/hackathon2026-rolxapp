use std::future::Future;
use std::ops::Deref;
use std::pin::Pin;
use std::rc::Rc;

use rolx_core::session::Session;
use rolx_core::store::SessionStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginMethod {
    /// Sign in with the system browser, which returns to the app by itself.
    Browser,
    /// Sign in with a code shown by the app, entered on a web page.
    Device,
}

/// What a running login asks the user to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginPrompt {
    /// The browser could not be opened: `url` has to be opened by hand.
    ManualUrl(String),
    /// `code` has to be entered at `uri`.
    DeviceCode { uri: String, code: String },
}

pub type LoginFuture = Pin<Box<dyn Future<Output = Result<Session, String>>>>;

/// Everything the shared UI needs from the app it runs in.
pub trait Platform {
    fn session_store(&self) -> &dyn SessionStore;

    /// The RolX server, `None` if not configured yet.
    fn server_url(&self) -> Option<String>;

    /// Where the configuration lives, for the user to see.
    fn config_location(&self) -> Option<String>;

    /// Why the configuration could not be loaded, if so.
    fn config_error(&self) -> Option<String>;

    fn login_methods(&self) -> Vec<LoginMethod>;

    /// Runs a login up to the signed-in session. The session is not saved yet.
    fn login(&self, method: LoginMethod, on_prompt: Rc<dyn Fn(LoginPrompt)>) -> LoginFuture;

    fn http_client(&self) -> reqwest::Client;

    /// Opens `url` outside the app. Returns whether that worked.
    fn open_external(&self, url: &str) -> bool;
}

/// A shared [`Platform`], usable as a component prop and as a context.
#[derive(Clone)]
pub struct PlatformHandle(Rc<dyn Platform>);

impl PlatformHandle {
    pub fn new(platform: impl Platform + 'static) -> Self {
        Self(Rc::new(platform))
    }
}

impl Deref for PlatformHandle {
    type Target = dyn Platform;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

impl PartialEq for PlatformHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
