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
    /// Sign in with a button the platform draws itself, see [`Platform::mount_login_button`].
    GoogleButton,
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

/// Gets the outcome of a sign-in that started outside the app's own code. It may be called from
/// anywhere, also from a browser callback.
pub type LoginResultHandler = Rc<dyn Fn(Result<Session, String>)>;

/// Everything the shared UI needs from the app it runs in.
pub trait Platform {
    fn session_store(&self) -> &dyn SessionStore;

    /// The RolX server, `None` if not configured yet.
    fn server_url(&self) -> Option<String>;

    /// Whether the settings screen may change the server.
    fn can_change_server(&self) -> bool {
        false
    }

    /// The address as it would be stored, or why this platform does not accept it.
    fn validate_server_url(&self, url: &str) -> Result<String, String> {
        normalize_server_url(url)
    }

    /// Changes the server for this and later runs, if the platform allows it.
    fn set_server_url(&self, _url: &str) -> Result<(), String> {
        Err("The server cannot be changed here.".to_string())
    }

    /// Where the configuration lives, for the user to see.
    fn config_location(&self) -> Option<String>;

    /// Why the configuration could not be loaded, if so.
    fn config_error(&self) -> Option<String>;

    fn login_methods(&self) -> Vec<LoginMethod>;

    /// Runs a login up to the signed-in session. The session is not saved yet.
    fn login(&self, method: LoginMethod, on_prompt: Rc<dyn Fn(LoginPrompt)>) -> LoginFuture;

    /// Draws the [`LoginMethod::GoogleButton`] into the element `element_id`. The session is not
    /// saved yet when it reaches `on_result`.
    fn mount_login_button(&self, _element_id: &str, _on_result: LoginResultHandler) {}

    fn http_client(&self) -> reqwest::Client;

    /// Opens `url` outside the app. Returns whether that worked.
    fn open_external(&self, url: &str) -> bool;

    /// Puts `text` on the clipboard. Returns whether that worked.
    fn copy_text(&self, text: &str) -> bool;
}

/// A server address as the API calls expect it: with a scheme and no trailing slash.
pub fn normalize_server_url(input: &str) -> Result<String, String> {
    let url = input.trim().trim_end_matches('/');
    for scheme in ["https://", "http://"] {
        // A phone keyboard likes to capitalize the first letter.
        let host = url
            .get(..scheme.len())
            .filter(|start| start.eq_ignore_ascii_case(scheme))
            .map(|_| &url[scheme.len()..]);
        if let Some(host) = host.filter(|h| !h.is_empty() && !h.contains(char::is_whitespace)) {
            return Ok(format!("{scheme}{host}"));
        }
    }
    Err("The address must start with http:// or https://.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_urls_are_trimmed_of_spaces_and_trailing_slashes() {
        assert_eq!(
            normalize_server_url(" https://example.invalid/ ").as_deref(),
            Ok("https://example.invalid")
        );
        assert_eq!(
            normalize_server_url("http://10.0.2.2:8090").as_deref(),
            Ok("http://10.0.2.2:8090")
        );
        assert_eq!(
            normalize_server_url("Https://example.invalid").as_deref(),
            Ok("https://example.invalid")
        );
    }

    #[test]
    fn server_urls_need_a_scheme_and_a_host() {
        for input in [
            "",
            "example.invalid",
            "ftp://example.invalid",
            "https://",
            "https://a b",
        ] {
            assert!(normalize_server_url(input).is_err(), "{input}");
        }
    }
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
