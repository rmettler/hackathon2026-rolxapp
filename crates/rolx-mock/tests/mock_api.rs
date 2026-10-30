use chrono::{Duration, Utc};
use reqwest::StatusCode;
use rolx_mock::{
    DEVICE_CLIENT_DENIED, DEVICE_CLIENT_EXPIRED, DEVICE_CLIENT_SLOW_DOWN, MOCK_ID_TOKEN,
    MOCK_USER_ID, MockConfig, RunningMock, bearer_token_expiring_at,
};
use serde_json::Value;

const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

async fn mock() -> RunningMock {
    rolx_mock::spawn(MockConfig::default()).await.unwrap()
}

fn valid_token() -> String {
    bearer_token_expiring_at(Utc::now() + Duration::hours(1))
}

async fn get_range(
    mock: &RunningMock,
    user: &str,
    range: &str,
    token: Option<&str>,
) -> reqwest::Response {
    let url = format!("{}/api/v1/WorkRecord/{user}/range/{range}", mock.base_url());
    let mut request = reqwest::Client::new().get(url);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    request.send().await.unwrap()
}

#[tokio::test]
async fn binds_to_loopback() {
    assert!(mock().await.base_url().starts_with("http://127.0.0.1:"));
}

#[tokio::test]
async fn range_is_half_open_with_one_record_per_day() {
    let mock = mock().await;
    let response = get_range(
        &mock,
        MOCK_USER_ID,
        "2026-10-30..2026-11-02",
        Some(&valid_token()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let records: Vec<Value> = response.json().await.unwrap();
    let dates: Vec<&str> = records
        .iter()
        .map(|r| r["date"].as_str().unwrap())
        .collect();
    assert_eq!(dates, ["2026-10-30", "2026-10-31", "2026-11-01"]);
}

#[tokio::test]
async fn fixtures_are_deterministic_and_differ_by_day() {
    let mock = mock().await;
    let token = valid_token();
    let fetch = async |range: &str| -> Value {
        get_range(&mock, MOCK_USER_ID, range, Some(&token))
            .await
            .json()
            .await
            .unwrap()
    };

    let friday = fetch("2026-10-30..2026-10-31").await;
    assert_eq!(friday, fetch("2026-10-30..2026-10-31").await);
    assert!(!friday[0]["entries"].as_array().unwrap().is_empty());
    assert!(friday[0]["entries"][0]["duration"].is_i64());

    let saturday = fetch("2026-10-31..2026-11-01").await;
    assert!(saturday[0]["entries"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn range_rejects_missing_expired_and_foreign_access() {
    let mock = mock().await;
    let range = "2026-10-30..2026-10-31";
    let expired = bearer_token_expiring_at(Utc::now() - Duration::seconds(1));
    let foreign = "00000000-0000-4000-8000-000000000002";

    let status = async |user, token| get_range(&mock, user, range, token).await.status();
    assert_eq!(status(MOCK_USER_ID, None).await, StatusCode::UNAUTHORIZED);
    assert_eq!(
        status(MOCK_USER_ID, Some("unknown")).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        status(MOCK_USER_ID, Some(&expired)).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        status(foreign, Some(&valid_token())).await,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn invalid_range_is_not_found() {
    let mock = mock().await;
    let response = get_range(
        &mock,
        MOCK_USER_ID,
        "yesterday..today",
        Some(&valid_token()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sign_in_info_needs_no_token() {
    let mock = mock().await;
    let info: Value = reqwest::get(format!("{}/api/v1/SignIn/info", mock.base_url()))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(info["googleClientId"].is_string());
    assert!(info["installationId"].is_string());
}

#[tokio::test]
async fn auth_redirects_back_with_code_and_encoded_state() {
    let mock = mock().await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let response = client
        .get(format!("{}/google/auth", mock.base_url()))
        .query(&[
            ("redirect_uri", "http://127.0.0.1:1234/"),
            ("state", "a b&c"),
        ])
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FOUND);
    let location = response.headers()["location"].to_str().unwrap();
    assert!(location.starts_with("http://127.0.0.1:1234/?code="));
    assert!(location.ends_with("&state=a+b%26c"));
}

async fn post_token(mock: &RunningMock, form: &[(&str, &str)]) -> (StatusCode, Value) {
    let response = reqwest::Client::new()
        .post(format!("{}/google/token", mock.base_url()))
        .form(form)
        .send()
        .await
        .unwrap();
    (response.status(), response.json().await.unwrap())
}

#[tokio::test]
async fn auth_code_grant_needs_the_code_and_a_verifier() {
    let mock = mock().await;
    let grant = ("grant_type", "authorization_code");

    let (status, body) = post_token(
        &mock,
        &[grant, ("code", "mock-auth-code"), ("code_verifier", "v")],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id_token"], MOCK_ID_TOKEN);

    let (status, body) = post_token(&mock, &[grant, ("code", "mock-auth-code")]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid_grant");

    let (_, body) = post_token(&mock, &[grant, ("code", "wrong"), ("code_verifier", "v")]).await;
    assert_eq!(body["error"], "invalid_grant");
}

async fn device_code(mock: &RunningMock, client_id: &str) -> String {
    let body: Value = reqwest::Client::new()
        .post(format!("{}/google/device/code", mock.base_url()))
        .form(&[("client_id", client_id), ("scope", "openid")])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        body["verification_url"]
            .as_str()
            .unwrap()
            .contains("/google/device")
    );
    body["device_code"].as_str().unwrap().to_string()
}

async fn poll(mock: &RunningMock, device_code: &str) -> Value {
    let form = [("grant_type", DEVICE_GRANT), ("device_code", device_code)];
    post_token(mock, &form).await.1
}

#[tokio::test]
async fn device_grant_is_pending_twice_then_succeeds() {
    let mock = mock().await;
    let code = device_code(&mock, "any").await;

    assert_eq!(poll(&mock, &code).await["error"], "authorization_pending");
    assert_eq!(poll(&mock, &code).await["error"], "authorization_pending");
    assert_eq!(poll(&mock, &code).await["id_token"], MOCK_ID_TOKEN);
}

#[tokio::test]
async fn device_grant_scenarios_follow_the_client_id() {
    let mock = mock().await;

    let code = device_code(&mock, DEVICE_CLIENT_SLOW_DOWN).await;
    assert_eq!(poll(&mock, &code).await["error"], "slow_down");
    assert_eq!(poll(&mock, &code).await["error"], "authorization_pending");

    let code = device_code(&mock, DEVICE_CLIENT_EXPIRED).await;
    assert_eq!(poll(&mock, &code).await["error"], "expired_token");

    let code = device_code(&mock, DEVICE_CLIENT_DENIED).await;
    assert_eq!(poll(&mock, &code).await["error"], "access_denied");
}

#[tokio::test]
async fn unknown_device_code_is_rejected() {
    let mock = mock().await;
    assert_eq!(poll(&mock, "unknown").await["error"], "invalid_grant");
}
