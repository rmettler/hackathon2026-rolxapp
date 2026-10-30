//! The host against a real upstream: the mock, and a small echo server to look at what arrives.

use std::fs;
use std::net::SocketAddr;

use axum::Router;
use axum::body::Bytes;
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::IntoResponse;
use axum::routing::any;
use chrono::{Duration, Utc};
use reqwest::redirect::Policy;
use rolx_mock::{MOCK_USER_ID, MockConfig, RunningMock, bearer_token_expiring_at};
use rolxapp_webhost::Config;
use serde_json::Value;
use tempfile::TempDir;
use tokio::net::TcpListener;

async fn serve(app: Router) -> SocketAddr {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await });
    address
}

fn bundle() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("index.html"), "<html>the app</html>").unwrap();
    fs::write(dir.path().join("app.js"), "// a file").unwrap();
    dir
}

/// A host in front of `server`. The returned guard keeps the bundle alive.
async fn host(server: &str) -> (String, TempDir) {
    let dir = bundle();
    let app = rolxapp_webhost::app(Config {
        server: server.to_string(),
        bundle_dir: dir.path().to_path_buf(),
        google_client_id: Some("fake-client-id".to_string()),
    })
    .unwrap();
    (format!("http://{}", serve(app).await), dir)
}

async fn mock() -> RunningMock {
    rolx_mock::spawn(MockConfig::default()).await.unwrap()
}

/// Answers with everything it was sent, as JSON.
async fn echo_server() -> String {
    async fn echo(uri: Uri, headers: HeaderMap, body: Bytes) -> impl IntoResponse {
        let seen: Vec<(String, String)> = headers
            .iter()
            .map(|(name, value)| {
                (
                    name.to_string(),
                    value.to_str().unwrap_or_default().to_string(),
                )
            })
            .collect();
        (
            StatusCode::IM_A_TEAPOT,
            [
                ("x-from-upstream", "kept"),
                ("access-control-allow-origin", "*"),
                ("access-control-allow-credentials", "true"),
                ("set-cookie", "tracker=1"),
                ("content-security-policy", "default-src *"),
                ("connection", "x-private-hop"),
                ("x-private-hop", "dropped"),
                ("keep-alive", "timeout=5"),
            ],
            axum::Json(serde_json::json!({
                "uri": uri.to_string(),
                "headers": seen,
                "body": String::from_utf8_lossy(&body),
            })),
        )
    }
    let address = serve(Router::new().fallback(any(echo))).await;
    format!("http://{address}")
}

fn sent_header<'a>(echo: &'a Value, name: &str) -> Option<&'a str> {
    echo["headers"]
        .as_array()?
        .iter()
        .find(|pair| pair[0] == name)
        .and_then(|pair| pair[1].as_str())
}

#[tokio::test]
async fn forwards_the_api_with_its_status_and_body() {
    let mock = mock().await;
    let (host, _dir) = host(mock.base_url()).await;

    let response = reqwest::get(format!("{host}/api/v1/SignIn/info"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK.as_u16());
    let info: Value = response.json().await.unwrap();
    assert_eq!(info["googleClientId"], "mock-client-id");
}

#[tokio::test]
async fn forwards_the_authorization_and_passes_rejections_on() {
    let mock = mock().await;
    let (host, _dir) = host(mock.base_url()).await;
    let url = format!("{host}/api/v1/WorkRecord/{MOCK_USER_ID}/range/2026-10-30..2026-10-31");
    let client = reqwest::Client::new();

    let without = client.get(&url).send().await.unwrap();
    assert_eq!(without.status(), StatusCode::UNAUTHORIZED.as_u16());

    let token = bearer_token_expiring_at(Utc::now() + Duration::hours(1));
    let with = client.get(&url).bearer_auth(token).send().await.unwrap();
    assert_eq!(with.status(), StatusCode::OK.as_u16());
    let records: Value = with.json().await.unwrap();
    assert_eq!(records[0]["date"], "2026-10-30");
}

#[tokio::test]
async fn forwards_a_post_body_query_and_status() {
    let echo = echo_server().await;
    let (host, _dir) = host(&echo).await;

    let response = reqwest::Client::new()
        .post(format!("{host}/api/v1/SignIn?a=b"))
        .body("the body")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::IM_A_TEAPOT.as_u16());
    let seen: Value = response.json().await.unwrap();
    assert_eq!(seen["uri"], "/api/v1/SignIn?a=b");
    assert_eq!(seen["body"], "the body");
}

#[tokio::test]
async fn rewrites_host_and_strips_hop_by_hop_headers_both_ways() {
    let echo = echo_server().await;
    let upstream_host = echo.trim_start_matches("http://").to_string();
    let (host, _dir) = host(&echo).await;

    let response = reqwest::Client::new()
        .get(format!("{host}/api/x"))
        .header("x-from-client", "kept")
        .header("proxy-authorization", "dropped")
        .header("te", "trailers")
        .header("cookie", "other-app=secret")
        .header("origin", "http://elsewhere.invalid")
        .header("x-forwarded-for", "1.2.3.4")
        .send()
        .await
        .unwrap();

    assert_eq!(response.headers()["x-from-upstream"], "kept");
    assert!(!response.headers().contains_key("x-private-hop"));
    assert!(!response.headers().contains_key("keep-alive"));

    let seen: Value = response.json().await.unwrap();
    assert_eq!(sent_header(&seen, "cookie"), None);
    assert_eq!(sent_header(&seen, "origin"), None);
    assert_eq!(sent_header(&seen, "x-forwarded-for"), None);
    assert_eq!(sent_header(&seen, "host"), Some(upstream_host.as_str()));
    assert_eq!(sent_header(&seen, "x-from-client"), Some("kept"));
    assert_eq!(sent_header(&seen, "proxy-authorization"), None);
    assert_eq!(sent_header(&seen, "te"), None);
}

#[tokio::test]
async fn api_answers_carry_no_cors_no_cookies_and_a_policy_that_lets_nothing_run() {
    let echo = echo_server().await;
    let (host, _dir) = host(&echo).await;

    let response = reqwest::get(format!("{host}/api/x")).await.unwrap();

    let headers = response.headers();
    assert!(!headers.contains_key("access-control-allow-origin"));
    assert!(!headers.contains_key("access-control-allow-credentials"));
    assert!(!headers.contains_key("set-cookie"));
    assert_eq!(
        headers["content-security-policy"],
        "default-src 'none'; sandbox"
    );
}

#[tokio::test]
async fn methods_a_rest_api_does_not_mean_are_not_forwarded() {
    let echo = echo_server().await;
    let (host, _dir) = host(&echo).await;
    let client = reqwest::Client::new();

    for method in ["TRACE", "OPTIONS"] {
        let method = reqwest::Method::from_bytes(method.as_bytes()).unwrap();
        let response = client
            .request(method.clone(), format!("{host}/api/x"))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::METHOD_NOT_ALLOWED.as_u16(),
            "{method}"
        );
    }
}

#[tokio::test]
async fn does_not_follow_upstream_redirects() {
    async fn redirect() -> impl IntoResponse {
        (
            StatusCode::FOUND,
            [(header::LOCATION, "http://elsewhere.invalid/")],
        )
    }
    let upstream = serve(Router::new().fallback(any(redirect))).await;
    let (host, _dir) = host(&format!("http://{upstream}")).await;
    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .build()
        .unwrap();

    let response = client.get(format!("{host}/api/x")).send().await.unwrap();

    assert_eq!(response.status(), StatusCode::FOUND.as_u16());
    assert_eq!(response.headers()["location"], "http://elsewhere.invalid/");
}

#[tokio::test]
async fn an_api_miss_stays_a_404_and_never_gets_the_app() {
    let mock = mock().await;
    let (host, _dir) = host(mock.base_url()).await;

    for path in ["/api", "/api/", "/api/v1/nothing"] {
        let response = reqwest::get(format!("{host}{path}")).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND.as_u16(), "{path}");
        assert!(
            !response.text().await.unwrap().contains("the app"),
            "{path}"
        );
    }
}

#[tokio::test]
async fn an_unreachable_upstream_is_a_bad_gateway_not_the_app() {
    let (host, _dir) = host("http://127.0.0.1:1").await;

    let response = reqwest::get(format!("{host}/api/v1/SignIn/info"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY.as_u16());
}

#[tokio::test]
async fn only_api_paths_are_forwarded() {
    let echo = echo_server().await;
    let (host, _dir) = host(&echo).await;

    let response = reqwest::get(format!("{host}/apiary")).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK.as_u16());
    assert!(response.text().await.unwrap().contains("the app"));
}

#[tokio::test]
async fn serves_files_and_falls_back_to_the_app_for_routes() {
    let mock = mock().await;
    let (host, _dir) = host(mock.base_url()).await;

    let file = reqwest::get(format!("{host}/app.js")).await.unwrap();
    assert!(file.text().await.unwrap().contains("a file"));

    for path in ["/", "/day/2026-10-30"] {
        let page = reqwest::get(format!("{host}{path}")).await.unwrap();
        assert_eq!(page.status(), StatusCode::OK.as_u16(), "{path}");
        assert!(page.text().await.unwrap().contains("the app"), "{path}");
    }
}

#[tokio::test]
async fn config_json_hands_out_the_client_id() {
    let mock = mock().await;
    let (host, _dir) = host(mock.base_url()).await;

    let config: Value = reqwest::get(format!("{host}/config.json"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(config["googleClientId"], "fake-client-id");
}

#[tokio::test]
async fn too_large_bodies_and_paths_leaving_the_api_are_refused() {
    let echo = echo_server().await;
    let (host, _dir) = host(&echo).await;
    let client = reqwest::Client::new();

    let large = client
        .post(format!("{host}/api/x"))
        .body(vec![b'x'; 1024 * 1024 + 1])
        .send()
        .await
        .unwrap();
    assert_eq!(large.status(), StatusCode::PAYLOAD_TOO_LARGE.as_u16());

    // reqwest would normalize a plain `..` or `%2e%2e/` away before sending.
    let escaping = client
        .get(format!("{host}/api/%2e%2e%2fsecret"))
        .send()
        .await
        .unwrap();
    assert_eq!(escaping.status(), StatusCode::BAD_REQUEST.as_u16());
}

/// Sends `path` exactly as given, as a hostile client would, and returns the first line of the answer.
async fn raw_get(host: &str, path: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let address = host.trim_start_matches("http://");
    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).await.unwrap();
    answer.lines().next().unwrap_or_default().to_string()
}

#[tokio::test]
async fn literal_dots_from_a_raw_client_never_reach_the_upstream() {
    let echo = echo_server().await;
    let (host, _dir) = host(&echo).await;

    for path in [
        "/api/../secret",
        "/api/%2e%2e/secret",
        "/api/v1/..%2f..%2fsecret",
    ] {
        let status_line = raw_get(&host, path).await;
        assert!(status_line.contains("400"), "{path}: {status_line}");
    }
}

#[tokio::test]
async fn sends_no_wildcard_cors_and_the_security_headers() {
    let mock = mock().await;
    let (host, _dir) = host(mock.base_url()).await;

    let response = reqwest::Client::new()
        .get(format!("{host}/"))
        .header("origin", "http://elsewhere.invalid")
        .send()
        .await
        .unwrap();

    assert!(
        !response
            .headers()
            .contains_key("access-control-allow-origin")
    );
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    let policy = response.headers()["content-security-policy"]
        .to_str()
        .unwrap();
    assert!(policy.contains("default-src 'self'"), "{policy}");
    assert!(!policy.contains('*') || policy.contains("https://*.googleusercontent.com"));
}
