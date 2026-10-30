//! Renders the whole app to HTML with a fake platform.

use std::cell::RefCell;
use std::rc::Rc;

use chrono::Duration;
use dioxus::prelude::*;
use rolx_core::session::Session;
use rolx_core::store::{SessionStore, StoreError};

use crate::state::tests::session_expiring_in;
use crate::state::{AppState, Route};
use crate::{App, LoginFuture, LoginMethod, LoginPrompt, Platform, PlatformHandle};

struct FakePlatform {
    session: RefCell<Option<Session>>,
}

impl SessionStore for FakePlatform {
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

impl Platform for FakePlatform {
    fn session_store(&self) -> &dyn SessionStore {
        self
    }

    fn server_url(&self) -> Option<String> {
        Some("http://127.0.0.1:1".into())
    }

    fn config_location(&self) -> Option<String> {
        None
    }

    fn config_error(&self) -> Option<String> {
        None
    }

    fn login_methods(&self) -> Vec<LoginMethod> {
        vec![LoginMethod::Browser, LoginMethod::Device]
    }

    fn login(&self, _method: LoginMethod, _on_prompt: Rc<dyn Fn(LoginPrompt)>) -> LoginFuture {
        Box::pin(async { Err("not used".to_string()) })
    }

    fn http_client(&self) -> reqwest::Client {
        reqwest::Client::new()
    }

    fn open_external(&self, _url: &str) -> bool {
        false
    }
}

fn render_app(session: Option<Session>) -> String {
    #[component]
    fn Root(platform: PlatformHandle) -> Element {
        rsx! { App { platform } }
    }

    let platform = PlatformHandle::new(FakePlatform {
        session: RefCell::new(session),
    });
    let mut dom = VirtualDom::new_with_props(Root, RootProps { platform });
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn without_a_session_the_login_shows_each_method() {
    let html = render_app(None);

    assert!(html.contains("Sign in with browser"), "{html}");
    assert!(html.contains("Sign in with a code"), "{html}");
    assert!(!html.contains("Signed in as"), "{html}");
}

#[test]
fn with_a_session_the_day_shows_the_user() {
    let html = render_app(Some(session_expiring_in(Duration::hours(1))));

    assert!(html.contains("Signed in as Test User"), "{html}");
    assert!(!html.contains("Sign in with browser"), "{html}");
}

#[test]
fn with_an_expired_session_the_login_shows() {
    let html = render_app(Some(session_expiring_in(Duration::seconds(-1))));

    assert!(html.contains("Sign in with browser"), "{html}");
}

#[test]
fn settings_show_the_server() {
    #[component]
    fn Root() -> Element {
        use_context_provider(|| {
            PlatformHandle::new(FakePlatform {
                session: RefCell::new(None),
            })
        });
        use_context_provider(|| AppState {
            route: Signal::new(Route::Settings),
            session: Signal::new(None),
        });
        rsx! { crate::views::SettingsView {} }
    }

    let mut dom = VirtualDom::new(Root);
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);

    assert!(html.contains("Server: http://127.0.0.1:1"), "{html}");
}
