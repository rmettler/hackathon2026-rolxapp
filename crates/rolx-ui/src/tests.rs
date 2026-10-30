//! Renders the whole app to HTML with a fake platform.

use std::cell::RefCell;
use std::rc::Rc;

use chrono::{Duration, NaiveDate};
use dioxus::prelude::*;
use rolx_core::clock;
use rolx_core::session::{BearerToken, Session};
use rolx_core::store::{SessionStore, StoreError};

use crate::day_model::{LoadError, fresh_session, load_day};
use crate::state::tests::session_expiring_in;
use crate::state::{AppState, Route};
use crate::views::end_session;
use crate::{App, LoginFuture, LoginMethod, LoginPrompt, Platform, PlatformHandle};

struct FakePlatform {
    session: RefCell<Option<Session>>,
    server_url: String,
}

impl FakePlatform {
    fn new(session: Option<Session>) -> Self {
        Self {
            session: RefCell::new(session),
            server_url: "http://127.0.0.1:1".to_string(),
        }
    }
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
        Some(self.server_url.clone())
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

    let platform = PlatformHandle::new(FakePlatform::new(session));
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
        use_context_provider(|| PlatformHandle::new(FakePlatform::new(None)));
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

fn platform_on(server_url: &str) -> PlatformHandle {
    PlatformHandle::new(FakePlatform {
        session: RefCell::new(None),
        server_url: server_url.to_string(),
    })
}

fn mock_session(token: String, lifetime: Duration) -> Session {
    Session {
        bearer_token: BearerToken::from_string(token),
        user_id: rolx_mock::MOCK_USER_ID.to_string(),
        ..session_expiring_in(lifetime)
    }
}

fn valid_mock_session() -> Session {
    let expires = clock::now() + Duration::hours(1);
    mock_session(
        rolx_mock::bearer_token_expiring_at(expires),
        Duration::hours(1),
    )
}

fn friday() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 30).unwrap()
}

async fn mock() -> rolx_mock::RunningMock {
    rolx_mock::spawn(rolx_mock::MockConfig::default())
        .await
        .unwrap()
}

#[tokio::test]
async fn a_day_loads_from_the_server() {
    let mock = mock().await;
    let platform = platform_on(mock.base_url());

    let day = load_day(&platform, &valid_mock_session(), friday())
        .await
        .unwrap();

    assert_eq!(day.rows.len(), 2);
    assert_eq!(day.total, "7:30");
}

#[tokio::test]
async fn a_rejected_token_is_session_rejected() {
    let mock = mock().await;
    let platform = platform_on(mock.base_url());
    let session = mock_session("rejected".to_string(), Duration::hours(1));

    let result = load_day(&platform, &session, friday()).await;

    assert_eq!(result, Err(LoadError::SessionRejected));
}

#[tokio::test]
async fn an_unreachable_server_can_be_retried_and_the_message_has_no_url() {
    let platform = platform_on("http://127.0.0.1:1");

    let result = load_day(&platform, &valid_mock_session(), friday()).await;

    let Err(LoadError::Failed(message)) = result else {
        panic!("expected a failure, got {result:?}");
    };
    assert!(!message.contains("127.0.0.1"), "{message}");
    assert!(!message.contains(rolx_mock::MOCK_USER_ID), "{message}");
}

#[tokio::test]
async fn a_near_expiry_session_is_extended_and_saved() {
    let mock = mock().await;
    let platform = platform_on(mock.base_url());
    let expires = clock::now() + Duration::minutes(5);
    let old = rolx_mock::bearer_token_expiring_at(expires);
    let session = mock_session(old.clone(), Duration::minutes(5));

    let fresh = fresh_session(&platform, session).await.unwrap();

    assert_ne!(fresh.bearer_token.as_str(), old);
    let saved = platform.session_store().load().unwrap().unwrap();
    assert_eq!(saved.bearer_token.as_str(), fresh.bearer_token.as_str());
}

#[tokio::test]
async fn a_rejected_extension_is_session_rejected_and_an_unreachable_one_can_be_retried() {
    let mock = mock().await;
    let rejected = mock_session("rejected".to_string(), Duration::minutes(5));
    let result = fresh_session(&platform_on(mock.base_url()), rejected).await;
    assert_eq!(result.unwrap_err(), LoadError::SessionRejected);

    let near_expiry = mock_session("any".to_string(), Duration::minutes(5));
    let result = fresh_session(&platform_on("http://127.0.0.1:1"), near_expiry).await;
    assert!(matches!(result, Err(LoadError::Failed(_))));
}

fn end_session_in_runtime(stored: Session, current: Session) -> Option<Session> {
    let platform = platform_on("http://127.0.0.1:1");
    platform.session_store().save(&stored).unwrap();
    fn host((platform, current): (PlatformHandle, Session)) -> Element {
        use_hook(|| {
            end_session(
                &platform,
                AppState {
                    route: Signal::new(Route::Day),
                    session: Signal::new(Some(current)),
                },
            )
        });
        rsx! {}
    }
    VirtualDom::new_with_props(host, (platform.clone(), current)).rebuild_in_place();
    platform.session_store().load().unwrap()
}

#[test]
fn ending_a_session_deletes_it_from_the_store() {
    let session = valid_mock_session();
    assert!(end_session_in_runtime(session.clone(), session).is_none());
}

#[test]
fn ending_a_session_keeps_a_newer_one_in_the_store() {
    let newer = mock_session("signed-in-again".to_string(), Duration::hours(1));
    let stored = end_session_in_runtime(newer, valid_mock_session()).unwrap();
    assert_eq!(stored.bearer_token.as_str(), "signed-in-again");
}
