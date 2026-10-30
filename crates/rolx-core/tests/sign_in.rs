//! `api::sign_in` against the in-process mock server.

use chrono::{Duration, Utc};
use rolx_core::api::sign_in::{self, InfoError, SignInError, SignInExtensionError};
use rolx_core::session::BearerToken;
use rolx_mock::{MOCK_ID_TOKEN, MOCK_USER_ID, MockConfig, bearer_token_expiring_at};

fn token_expiring_in(lifetime: Duration) -> BearerToken {
    BearerToken::from_string(bearer_token_expiring_at(Utc::now() + lifetime))
}

#[tokio::test]
async fn info_answers_without_a_session() {
    let mock = rolx_mock::spawn(MockConfig::default()).await.unwrap();

    let info = sign_in::info(&reqwest::Client::new(), mock.base_url())
        .await
        .unwrap();

    assert_eq!(info.google_client_id, "mock-client-id");
}

#[tokio::test]
async fn info_fails_when_nothing_answers_or_the_address_is_not_a_rolx_server() {
    let client = reqwest::Client::new();
    let result = sign_in::info(&client, "http://127.0.0.1:1").await;
    assert!(matches!(result, Err(InfoError::Http(_))));

    let mock = rolx_mock::spawn(MockConfig::default()).await.unwrap();
    let not_rolx = format!("{}/nothing-here", mock.base_url());
    let result = sign_in::info(&client, &not_rolx).await;
    assert!(matches!(result, Err(InfoError::Status(404))));
}

#[tokio::test]
async fn sign_in_returns_a_session_for_an_accepted_token() {
    let mock = rolx_mock::spawn(MockConfig::default()).await.unwrap();

    let session = sign_in::sign_in(&reqwest::Client::new(), mock.base_url(), MOCK_ID_TOKEN)
        .await
        .unwrap();

    assert_eq!(session.user_id, MOCK_USER_ID);
    assert_eq!(session.user_first_name, "Mock");
    assert!(!session.is_near_expiry());
}

#[tokio::test]
async fn sign_in_with_an_empty_401_is_unauthorized() {
    let mock = rolx_mock::spawn(MockConfig::default()).await.unwrap();

    let result = sign_in::sign_in(&reqwest::Client::new(), mock.base_url(), "not-accepted").await;

    assert!(matches!(result, Err(SignInError::Unauthorized)));
}

#[tokio::test]
async fn token_lifetime_of_the_mock_reaches_the_session() {
    let config = MockConfig {
        token_lifetime: Duration::minutes(5),
        ..MockConfig::default()
    };
    let mock = rolx_mock::spawn(config).await.unwrap();

    let session = sign_in::sign_in(&reqwest::Client::new(), mock.base_url(), MOCK_ID_TOKEN)
        .await
        .unwrap();

    assert!(session.is_near_expiry());
    assert!(!session.is_expired());
}

#[tokio::test]
async fn extend_returns_a_new_session() {
    let mock = rolx_mock::spawn(MockConfig::default()).await.unwrap();
    let old = token_expiring_in(Duration::minutes(1));

    let session = sign_in::extend(&reqwest::Client::new(), mock.base_url(), &old)
        .await
        .unwrap();

    assert_ne!(session.bearer_token.as_str(), old.as_str());
    assert!(!session.is_near_expiry());
}

#[tokio::test]
async fn extend_with_an_expired_or_unknown_token_is_session_expired() {
    let mock = rolx_mock::spawn(MockConfig::default()).await.unwrap();
    let client = reqwest::Client::new();

    let expired = token_expiring_in(Duration::seconds(-1));
    let unknown = BearerToken::from_string("unknown".into());

    for token in [expired, unknown] {
        let result = sign_in::extend(&client, mock.base_url(), &token).await;
        assert!(matches!(result, Err(SignInExtensionError::SessionExpired)));
    }
}
