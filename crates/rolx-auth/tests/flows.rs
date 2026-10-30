//! Both login flows against the in-process mock of Google's endpoints.

use std::sync::Mutex;
use std::time::Instant;

use rolx_auth::{AuthPrompt, Credentials, Endpoints, Flow, LoginError, login};
use rolx_mock::{
    DEVICE_CLIENT_DENIED, DEVICE_CLIENT_EXPIRED, DEVICE_CLIENT_SLOW_DOWN, MOCK_ID_TOKEN,
    MockConfig, RunningMock,
};

async fn mock() -> RunningMock {
    let config = MockConfig {
        device_poll_interval_secs: 0,
        ..MockConfig::default()
    };
    rolx_mock::spawn(config).await.unwrap()
}

fn endpoints(mock: &RunningMock) -> Endpoints {
    let base = format!("{}/google", mock.base_url());
    Endpoints {
        authorization: format!("{base}/auth"),
        token: format!("{base}/token"),
        device_authorization: format!("{base}/device/code"),
    }
}

fn credentials(id: &str) -> Credentials {
    Credentials {
        id: id.to_string(),
        secret: Some("mock-secret".to_string()),
    }
}

/// Plays the browser: follows the mock's redirect back to the app's loopback listener.
#[derive(Default)]
struct BrowserPrompt {
    can_open: bool,
    opened: Mutex<Vec<String>>,
    manual: Mutex<Vec<String>>,
    device_codes: Mutex<Vec<(String, String)>>,
}

impl BrowserPrompt {
    fn visit(url: &str) {
        let url = url.to_string();
        tokio::spawn(async move { reqwest::get(url).await });
    }
}

impl AuthPrompt for BrowserPrompt {
    fn open_url(&self, url: &str) -> bool {
        if self.can_open {
            self.opened.lock().unwrap().push(url.to_string());
            Self::visit(url);
        }
        self.can_open
    }

    fn show_manual_url(&self, url: &str) {
        self.manual.lock().unwrap().push(url.to_string());
        Self::visit(url);
    }

    fn show_device_code(&self, uri: &str, code: &str) {
        self.device_codes
            .lock()
            .unwrap()
            .push((uri.to_string(), code.to_string()));
    }
}

async fn native_login(mock: &RunningMock, prompt: &BrowserPrompt) -> Result<String, LoginError> {
    let flow = Flow::NativeApp {
        user_email: Some("mock.user@example.invalid".to_string()),
    };
    login(
        &reqwest::Client::new(),
        flow,
        &credentials("mock-client"),
        &endpoints(mock),
        prompt,
    )
    .await
    .map(|token| token.as_str().to_string())
}

async fn device_login(
    mock: &RunningMock,
    client_id: &str,
) -> (Result<String, LoginError>, BrowserPrompt) {
    let prompt = BrowserPrompt::default();
    let result = login(
        &reqwest::Client::new(),
        Flow::Device,
        &credentials(client_id),
        &endpoints(mock),
        &prompt,
    )
    .await
    .map(|token| token.as_str().to_string());
    (result, prompt)
}

#[tokio::test]
async fn loopback_flow_gets_the_id_token_through_the_opened_browser() {
    let mock = mock().await;
    let prompt = BrowserPrompt {
        can_open: true,
        ..BrowserPrompt::default()
    };

    let token = native_login(&mock, &prompt).await.unwrap();

    assert_eq!(token, MOCK_ID_TOKEN);
    let opened = prompt.opened.lock().unwrap();
    assert_eq!(opened.len(), 1);
    assert!(opened[0].contains("code_challenge_method=S256"));
    assert!(opened[0].contains("login_hint=mock.user%40example.invalid"));
    assert!(prompt.manual.lock().unwrap().is_empty());
}

#[tokio::test]
async fn loopback_flow_falls_back_to_the_manual_url() {
    let mock = mock().await;
    let prompt = BrowserPrompt::default();

    let token = native_login(&mock, &prompt).await.unwrap();

    assert_eq!(token, MOCK_ID_TOKEN);
    assert_eq!(prompt.manual.lock().unwrap().len(), 1);
}

/// Plays an attacker or a failing Google: answers the loopback listener with its own query.
struct ForgedRedirectPrompt {
    /// Query to send; `{state}` is replaced by the state of the authorization request.
    query: &'static str,
}

fn query_param(url: &str, name: &str) -> String {
    let value = url
        .split(['?', '&'])
        .find_map(|param| param.strip_prefix(&format!("{name}=")))
        .unwrap();
    urlencoding::decode(value).unwrap().into_owned()
}

impl AuthPrompt for ForgedRedirectPrompt {
    fn open_url(&self, url: &str) -> bool {
        let query = self.query.replace("{state}", &query_param(url, "state"));
        BrowserPrompt::visit(&format!("{}/?{query}", query_param(url, "redirect_uri")));
        true
    }

    fn show_manual_url(&self, _url: &str) {}

    fn show_device_code(&self, _uri: &str, _code: &str) {}
}

async fn login_with_forged_redirect(query: &'static str) -> String {
    let mock = mock().await;
    let result = login(
        &reqwest::Client::new(),
        Flow::NativeApp { user_email: None },
        &credentials("mock-client"),
        &endpoints(&mock),
        &ForgedRedirectPrompt { query },
    )
    .await;
    result.unwrap_err().to_string()
}

#[tokio::test]
async fn loopback_flow_rejects_a_redirect_with_the_wrong_state() {
    let error = login_with_forged_redirect("code=mock-auth-code&state=forged").await;
    assert!(error.contains("invalid state"), "{error}");
}

#[tokio::test]
async fn loopback_flow_rejects_a_redirect_without_state() {
    let error = login_with_forged_redirect("code=mock-auth-code").await;
    assert!(error.contains("invalid state"), "{error}");
}

#[tokio::test]
async fn loopback_flow_reports_an_authorization_error() {
    let error = login_with_forged_redirect("error=access_denied&state={state}").await;
    assert!(error.contains("access_denied"), "{error}");
}

#[tokio::test]
async fn device_flow_shows_the_code_and_succeeds_after_pending() {
    let mock = mock().await;

    let (result, prompt) = device_login(&mock, "mock-client").await;

    assert_eq!(result.unwrap(), MOCK_ID_TOKEN);
    let codes = prompt.device_codes.lock().unwrap();
    assert_eq!(codes.len(), 1);
    assert!(codes[0].0.ends_with("/google/device"));
    assert_eq!(codes[0].1, "MOCK-CODE");
}

#[tokio::test]
async fn device_flow_waits_longer_after_slow_down_and_succeeds() {
    let mock = mock().await;
    let start = Instant::now();

    let (result, _) = device_login(&mock, DEVICE_CLIENT_SLOW_DOWN).await;

    assert_eq!(result.unwrap(), MOCK_ID_TOKEN);
    assert!(start.elapsed().as_secs() >= 5, "RFC 8628 requires +5 s");
}

#[tokio::test]
async fn device_flow_reports_expiry_and_denial() {
    let mock = mock().await;

    let (expired, _) = device_login(&mock, DEVICE_CLIENT_EXPIRED).await;
    assert!(matches!(expired, Err(LoginError::Device(_))));
    assert!(expired.unwrap_err().to_string().contains("expired"));

    let (denied, _) = device_login(&mock, DEVICE_CLIENT_DENIED).await;
    assert!(denied.unwrap_err().to_string().contains("access_denied"));
}
