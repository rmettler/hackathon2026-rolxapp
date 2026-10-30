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
use crate::views::{
    LoginPhase, LoginStatus, Reachability, ServerChange, change_server, check_server, end_session,
    log_out,
};
use crate::{App, LoginFuture, LoginMethod, LoginPrompt, Platform, PlatformHandle};

struct FakePlatform {
    session: RefCell<Option<Session>>,
    server_url: RefCell<String>,
    can_change_server: bool,
    fail_set_server: bool,
}

impl FakePlatform {
    fn new(session: Option<Session>) -> Self {
        Self {
            session: RefCell::new(session),
            server_url: RefCell::new("http://127.0.0.1:1".to_string()),
            can_change_server: false,
            fail_set_server: false,
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
        Some(self.server_url.borrow().clone())
    }

    fn can_change_server(&self) -> bool {
        self.can_change_server
    }

    fn set_server_url(&self, url: &str) -> Result<(), String> {
        if self.fail_set_server {
            return Err("cannot save".to_string());
        }
        *self.server_url.borrow_mut() = url.to_string();
        Ok(())
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

    fn copy_text(&self, _text: &str) -> bool {
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
    assert!(!html.contains("Log out"), "{html}");
}

#[test]
fn with_a_session_the_day_shows_the_user() {
    let html = render_app(Some(session_expiring_in(Duration::hours(1))));

    assert!(html.contains("Test User"), "{html}");
    assert!(html.contains("Log out"), "{html}");
    assert!(!html.contains("Sign in with browser"), "{html}");
}

#[test]
fn with_an_expired_session_the_login_shows() {
    let html = render_app(Some(session_expiring_in(Duration::seconds(-1))));

    assert!(html.contains("Sign in with browser"), "{html}");
}

fn render_settings(can_change_server: bool) -> String {
    #[component]
    fn Root(can_change_server: bool) -> Element {
        use_context_provider(|| {
            PlatformHandle::new(FakePlatform {
                can_change_server,
                ..FakePlatform::new(None)
            })
        });
        use_context_provider(|| AppState {
            route: Signal::new(Route::Settings),
            session: Signal::new(None),
        });
        rsx! { crate::views::SettingsView {} }
    }

    let mut dom = VirtualDom::new_with_props(Root, RootProps { can_change_server });
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn settings_show_the_server() {
    let html = render_settings(false);
    assert!(html.contains("Server: http://127.0.0.1:1"), "{html}");
}

#[test]
fn the_server_form_shows_only_where_the_platform_allows_it() {
    assert!(!render_settings(false).contains("save-server"));
    assert!(render_settings(true).contains("save-server"));
}

fn platform_with_session() -> PlatformHandle {
    PlatformHandle::new(FakePlatform::new(Some(valid_mock_session())))
}

#[test]
fn a_new_server_ends_the_session_before_it_can_receive_the_token() {
    let platform = platform_with_session();

    let change = change_server(&platform, "https://other.invalid/");

    assert!(change.session_ended);
    assert_eq!(change.error, None);
    assert!(platform.session_store().load().unwrap().is_none());
    assert_eq!(
        platform.server_url().as_deref(),
        Some("https://other.invalid")
    );
}

#[test]
fn a_new_server_ends_a_session_held_only_in_memory() {
    // The store is empty, as after an earlier failure, but the app may still hold a session.
    let platform = PlatformHandle::new(FakePlatform::new(None));

    let change = change_server(&platform, "https://other.invalid");

    assert!(change.session_ended);
}

#[test]
fn saving_the_current_server_keeps_the_session() {
    let platform = platform_with_session();

    let change = change_server(&platform, "http://127.0.0.1:1/");

    assert_eq!(change, ServerChange::default());
    assert!(platform.session_store().load().unwrap().is_some());
}

#[test]
fn an_invalid_server_changes_nothing() {
    let platform = platform_with_session();

    let change = change_server(&platform, "not a url");

    assert!(change.error.is_some());
    assert!(!change.session_ended);
    assert!(platform.session_store().load().unwrap().is_some());
    assert_eq!(platform.server_url().as_deref(), Some("http://127.0.0.1:1"));
}

#[test]
fn a_server_that_cannot_be_saved_still_ends_the_session_and_reports_the_error() {
    let platform = PlatformHandle::new(FakePlatform {
        fail_set_server: true,
        ..FakePlatform::new(Some(valid_mock_session()))
    });

    let change = change_server(&platform, "https://other.invalid");

    assert!(change.session_ended);
    assert_eq!(change.error.as_deref(), Some("cannot save"));
    assert_eq!(platform.server_url().as_deref(), Some("http://127.0.0.1:1"));
}

fn platform_on(server_url: &str) -> PlatformHandle {
    PlatformHandle::new(FakePlatform {
        server_url: RefCell::new(server_url.to_string()),
        ..FakePlatform::new(None)
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

fn render_login_status(phase: LoginPhase) -> String {
    #[component]
    fn Root(phase: LoginPhase) -> Element {
        use_context_provider(|| PlatformHandle::new(FakePlatform::new(None)));
        rsx! { LoginStatus { phase } }
    }

    let mut dom = VirtualDom::new_with_props(Root, RootProps { phase });
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn login_status_shows_each_phase() {
    let idle = render_login_status(LoginPhase::Idle { error: None });
    assert!(idle.contains("Sign in to see"), "{idle}");

    let failed = render_login_status(LoginPhase::Idle {
        error: Some("denied".to_string()),
    });
    assert!(failed.contains("Sign-in failed: denied"), "{failed}");

    let waiting = render_login_status(LoginPhase::Waiting { prompt: None });
    assert!(waiting.contains("Waiting for you"), "{waiting}");

    let manual = render_login_status(LoginPhase::Waiting {
        prompt: Some(LoginPrompt::ManualUrl(
            "http://example.invalid/a".to_string(),
        )),
    });
    assert!(manual.contains("http://example.invalid/a"), "{manual}");

    let device = render_login_status(LoginPhase::Waiting {
        prompt: Some(LoginPrompt::DeviceCode {
            uri: "http://example.invalid/device".to_string(),
            code: "ABC-DEF".to_string(),
        }),
    });
    assert!(device.contains("ABC-DEF"), "{device}");
    assert!(device.contains("http://example.invalid/device"), "{device}");
    assert!(device.contains("Copy code"), "{device}");
    assert!(device.contains("Open browser"), "{device}");
}

#[tokio::test]
async fn the_server_check_tells_reachable_from_unreachable() {
    let mock = mock().await;
    assert_eq!(
        check_server(&platform_on(mock.base_url())).await,
        Reachability::Reachable
    );
    assert_eq!(
        check_server(&platform_on("http://127.0.0.1:1")).await,
        Reachability::Unreachable
    );
}

#[test]
fn logging_out_clears_the_store_and_the_session_and_goes_to_the_login() {
    let platform = platform_on("http://127.0.0.1:1");
    platform
        .session_store()
        .save(&valid_mock_session())
        .unwrap();
    fn host(platform: PlatformHandle) -> Element {
        use_hook(|| {
            let state = AppState {
                route: Signal::new(Route::Day),
                session: Signal::new(Some(valid_mock_session())),
            };
            log_out(&platform, state);
            assert_eq!(*state.route.peek(), Route::Login);
            assert!(state.session.peek().is_none());
        });
        rsx! {}
    }
    VirtualDom::new_with_props(host, platform.clone()).rebuild_in_place();

    assert!(platform.session_store().load().unwrap().is_none());
}
