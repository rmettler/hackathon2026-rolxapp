//! `ensure_fresh`, `RolxClient` and `get_range` against the in-process mock server.

use std::cell::RefCell;

use chrono::{Duration, NaiveDate};
use rolx_core::api::work_record::{self, WorkRecordError};
use rolx_core::client::{RolxClient, SessionError, ensure_fresh};
use rolx_core::clock;
use rolx_core::date_range::DateRange;
use rolx_core::session::{BearerToken, Session};
use rolx_core::store::{SessionStore, StoreError};
use rolx_mock::{MOCK_USER_ID, MockConfig, RunningMock, bearer_token_expiring_at};

/// Nothing listens here, so any request would fail.
const UNREACHABLE: &str = "http://127.0.0.1:1";

#[derive(Default)]
struct MemoryStore(RefCell<Option<Session>>);

impl SessionStore for MemoryStore {
    fn load(&self) -> Result<Option<Session>, StoreError> {
        Ok(self.0.borrow().clone())
    }

    fn save(&self, session: &Session) -> Result<(), StoreError> {
        *self.0.borrow_mut() = Some(session.clone());
        Ok(())
    }

    fn delete(&self) -> Result<(), StoreError> {
        *self.0.borrow_mut() = None;
        Ok(())
    }
}

fn session_with_token(token: String, lifetime: Duration) -> Session {
    Session {
        bearer_token: BearerToken::from_string(token),
        expires: clock::now() + lifetime,
        user_id: MOCK_USER_ID.into(),
        user_email: "mock.user@example.invalid".into(),
        user_first_name: "Mock".into(),
        user_last_name: "User".into(),
    }
}

fn session_expiring_in(lifetime: Duration) -> Session {
    let token = bearer_token_expiring_at(clock::now() + lifetime);
    session_with_token(token, lifetime)
}

async fn mock() -> RunningMock {
    rolx_mock::spawn(MockConfig::default()).await.unwrap()
}

fn friday() -> DateRange {
    DateRange::day(NaiveDate::from_ymd_opt(2026, 10, 30).unwrap())
}

#[tokio::test]
async fn valid_session_is_returned_without_a_request_or_a_save() {
    let store = MemoryStore::default();
    let session = session_expiring_in(Duration::hours(1));
    let token = session.bearer_token.as_str().to_string();

    let fresh = ensure_fresh(&reqwest::Client::new(), UNREACHABLE, session, &store)
        .await
        .unwrap();

    assert_eq!(fresh.bearer_token.as_str(), token);
    assert!(store.load().unwrap().is_none());
}

#[tokio::test]
async fn near_expiry_session_is_extended_and_saved() {
    let mock = mock().await;
    let store = MemoryStore::default();
    let session = session_expiring_in(Duration::minutes(5));
    let old_token = session.bearer_token.as_str().to_string();

    let fresh = ensure_fresh(&reqwest::Client::new(), mock.base_url(), session, &store)
        .await
        .unwrap();

    assert_ne!(fresh.bearer_token.as_str(), old_token);
    assert!(!fresh.is_near_expiry());
    let saved = store.load().unwrap().unwrap();
    assert_eq!(saved.bearer_token.as_str(), fresh.bearer_token.as_str());
}

#[tokio::test]
async fn expired_session_is_an_error_without_a_request() {
    let store = MemoryStore::default();
    let session = session_expiring_in(Duration::seconds(-1));

    let result = ensure_fresh(&reqwest::Client::new(), UNREACHABLE, session, &store).await;

    assert!(matches!(result, Err(SessionError::Expired)));
}

#[tokio::test]
async fn unauthorized_extension_is_expired_and_saves_nothing() {
    let mock = mock().await;
    let store = MemoryStore::default();
    let session = session_with_token("unknown".into(), Duration::minutes(5));

    let result = ensure_fresh(&reqwest::Client::new(), mock.base_url(), session, &store).await;

    assert!(matches!(result, Err(SessionError::Expired)));
    assert!(store.load().unwrap().is_none());
}

#[tokio::test]
async fn unreachable_server_is_an_extension_error() {
    let store = MemoryStore::default();
    let session = session_expiring_in(Duration::minutes(5));

    let result = ensure_fresh(&reqwest::Client::new(), UNREACHABLE, session, &store).await;

    assert!(matches!(result, Err(SessionError::Extension(_))));
}

#[tokio::test]
async fn client_from_an_empty_store_is_no_session() {
    let store = MemoryStore::default();

    let result =
        RolxClient::from_store(reqwest::Client::new(), UNREACHABLE.to_string(), &store).await;

    assert!(matches!(result, Err(SessionError::NoSession)));
}

#[tokio::test]
async fn client_reads_the_records_of_a_day() {
    let mock = mock().await;
    let store = MemoryStore::default();
    store
        .save(&session_expiring_in(Duration::hours(1)))
        .unwrap();

    let client =
        RolxClient::from_store(reqwest::Client::new(), mock.base_url().to_string(), &store)
            .await
            .unwrap();
    let records =
        work_record::get_range(&client.http, &client.server_url, &client.session, friday())
            .await
            .unwrap();

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].date, "2026-10-30");
    assert!(!records[0].entries.is_empty());
}

#[tokio::test]
async fn rejected_token_is_unauthorized_and_foreign_user_a_status_error() {
    let mock = mock().await;
    let http = reqwest::Client::new();

    let rejected = session_with_token("unknown".into(), Duration::hours(1));
    let result = work_record::get_range(&http, mock.base_url(), &rejected, friday()).await;
    assert!(matches!(result, Err(WorkRecordError::Unauthorized)));

    let mut foreign = session_expiring_in(Duration::hours(1));
    foreign.user_id = "00000000-0000-4000-8000-000000000002".into();
    let result = work_record::get_range(&http, mock.base_url(), &foreign, friday()).await;
    assert!(matches!(
        result,
        Err(WorkRecordError::Status(reqwest::StatusCode::FORBIDDEN))
    ));
}
