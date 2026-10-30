//! Serves the web app and forwards `/api/*` to the RolX server, so the browser sees one origin and
//! needs no CORS. Handles real bearer tokens: nothing here logs a header, a body or an API path, as
//! the path holds the user id.

use std::path::PathBuf;
use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{any, get};
use serde_json::json;
use tokio::time::timeout;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;

/// More than any request of the API needs.
const MAX_REQUEST_BODY: usize = 1024 * 1024;

/// Only the app's own files; the Google sign-in library and its frames are the one exception.
/// Local storage holds the bearer token, so a foreign script must not run on this origin. Inline
/// scripts are refused. `'unsafe-eval'` has to stay: the Dioxus components' `document::eval` runs
/// through `new Function`, and without it the wasm app panics on start.
const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; \
    script-src 'self' 'unsafe-eval' https://accounts.google.com/gsi/client; \
    style-src 'self' 'unsafe-inline' https://accounts.google.com/gsi/style; \
    frame-src https://accounts.google.com/gsi/; \
    connect-src 'self' https://accounts.google.com/gsi/; \
    img-src 'self' data: https://*.googleusercontent.com; \
    object-src 'none'; base-uri 'self'; frame-ancestors 'none'";

/// API answers are data: nothing in them may run, whatever the upstream says.
const API_CONTENT_SECURITY_POLICY: &str = "default-src 'none'; sandbox";

/// How long a client may take to send its request body.
const REQUEST_BODY_TIMEOUT: Duration = Duration::from_secs(30);

/// Headers of one connection, which RFC 9110 forbids to forward.
const HOP_BY_HOP: [&str; 9] = [
    "connection",
    "keep-alive",
    "proxy-connection",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

/// Request headers that are no business of the upstream: the browser's cookies for this host, an
/// origin that would only start CORS decisions there, and claims about the client that a proxy
/// in front of the server might believe.
const NOT_FORWARDED: [&str; 7] = [
    "cookie",
    "origin",
    "forwarded",
    "x-forwarded-for",
    "x-forwarded-host",
    "x-forwarded-proto",
    "x-real-ip",
];

/// The methods a REST API can mean. `CONNECT` and `TRACE` in particular are not forwarded.
const FORWARDED_METHODS: [Method; 6] = [
    Method::GET,
    Method::HEAD,
    Method::POST,
    Method::PUT,
    Method::PATCH,
    Method::DELETE,
];

#[derive(Debug, Clone)]
pub struct Config {
    /// The RolX server, like `https://rolx.example`, without a trailing slash.
    pub server: String,
    /// The directory with the web app's `index.html`. Without one, only the API and `/config.json`
    /// are served, as behind `dx serve` in development.
    pub bundle_dir: Option<PathBuf>,
    /// Handed to the browser in `/config.json`.
    pub google_client_id: Option<String>,
}

#[derive(Clone)]
struct Proxy {
    client: reqwest::Client,
    server: String,
    google_client_id: Option<String>,
}

/// The upstream client: bundled TLS roots, so a minimal container needs no CA files, and redirects
/// passed on to the browser rather than followed with its credentials.
fn upstream_client() -> reqwest::Result<reqwest::Client> {
    let roots = webpki_root_certs::TLS_SERVER_ROOT_CERTS
        .iter()
        .filter_map(|cert| reqwest::Certificate::from_der(cert.as_ref()).ok());
    reqwest::Client::builder()
        .tls_certs_only(roots)
        .redirect(reqwest::redirect::Policy::none())
        // Tokens go to `--server` only, not through a proxy from the environment.
        .no_proxy()
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(60))
        .build()
}

pub fn app(config: Config) -> reqwest::Result<Router> {
    // The process-wide choice between rustls' providers; an earlier call in tests is fine.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let proxy = Proxy {
        client: upstream_client()?,
        server: config.server.trim_end_matches('/').to_string(),
        google_client_id: config.google_client_id,
    };

    let api = Router::new()
        .route("/api", any(api_root))
        .route("/api/", any(api_root))
        .route("/api/{*rest}", any(forward))
        .layer(SetResponseHeaderLayer::overriding(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(API_CONTENT_SECURITY_POLICY),
        ));

    let routes = api.route("/config.json", get(config_json));
    let routes = match &config.bundle_dir {
        Some(dir) => {
            // Anything that is not a file is a client-side route of the app.
            let files = ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html")));
            routes.fallback_service(files)
        }
        None => routes,
    };

    Ok(routes
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(CONTENT_SECURITY_POLICY),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .with_state(proxy))
}

/// `/api` itself is no resource: it must not fall through to the app's `index.html`.
async fn api_root() -> StatusCode {
    StatusCode::NOT_FOUND
}

async fn config_json(State(proxy): State<Proxy>) -> Response {
    let body = Json(json!({ "googleClientId": proxy.google_client_id }));
    ([(header::CACHE_CONTROL, "no-store")], body).into_response()
}

/// A path that could leave `/api` once something behind this host normalizes it: any segment that
/// starts with `..`, however often it was encoded. Undecodable text is refused as well.
fn escapes_api(path: &str) -> bool {
    let mut decoded = path.replace('\\', "/");
    loop {
        if decoded.split('/').any(|segment| segment.starts_with("..")) {
            return true;
        }
        let Ok(next) = urlencoding::decode(&decoded) else {
            return true;
        };
        let next = next.replace('\\', "/");
        if next == decoded {
            return false;
        }
        decoded = next;
    }
}

fn is_hop_by_hop(name: &HeaderName, listed_in_connection: &[String]) -> bool {
    HOP_BY_HOP.contains(&name.as_str()) || listed_in_connection.iter().any(|h| h == name.as_str())
}

/// The headers a sender names in its `Connection` header are hop-by-hop as well.
fn connection_listed(headers: &HeaderMap) -> Vec<String> {
    headers
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(|name| name.trim().to_ascii_lowercase())
        .collect()
}

fn end_to_end(headers: &HeaderMap) -> impl Iterator<Item = (&HeaderName, &HeaderValue)> {
    let listed = connection_listed(headers);
    headers
        .iter()
        .filter(move |(name, _)| !is_hop_by_hop(name, &listed))
}

async fn forward(State(proxy): State<Proxy>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let path = parts.uri.path();
    if !FORWARDED_METHODS.contains(&parts.method) {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    if escapes_api(path) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let body = match timeout(REQUEST_BODY_TIMEOUT, to_bytes(body, MAX_REQUEST_BODY)).await {
        Ok(Ok(body)) => body,
        Ok(Err(_)) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
        Err(_) => return StatusCode::REQUEST_TIMEOUT.into_response(),
    };

    let path_and_query = parts.uri.path_and_query().map_or(path, |pq| pq.as_str());
    let url = format!("{}{path_and_query}", proxy.server);
    let method = parts.method.clone();

    let mut upstream = proxy.client.request(method.clone(), url);
    for (name, value) in end_to_end(&parts.headers) {
        // `Host` is the upstream's, and is set from the URL; the length is the one of this body.
        let replaced = name == header::HOST || name == header::CONTENT_LENGTH;
        if !replaced && !NOT_FORWARDED.contains(&name.as_str()) {
            upstream = upstream.header(name, value);
        }
    }
    if method != Method::GET && method != Method::HEAD {
        upstream = upstream.body(body);
    }

    let response = match upstream.send().await {
        Ok(response) => response,
        Err(error) => {
            // Without the URL, which holds the user id.
            tracing::warn!(error = %error.without_url(), %method, "upstream request failed");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };
    tracing::debug!(%method, status = response.status().as_u16(), "forwarded");

    let mut builder = Response::builder().status(response.status());
    if let Some(headers) = builder.headers_mut() {
        for (name, value) in end_to_end(response.headers()) {
            // No CORS here at all: the app is on this origin, and no other may read the API. Nor
            // any cookies, which the browser would keep for this host and all its other ports.
            let cross_site =
                name.as_str().starts_with("access-control-") || name == header::SET_COOKIE;
            if !cross_site {
                headers.append(name, value.clone());
            }
        }
    }
    builder
        .body(Body::from_stream(response.bytes_stream()))
        .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dots_leaving_the_api_are_found_also_when_encoded() {
        for path in [
            "/api/../secret",
            "/api/v1/../../secret",
            "/api/%2e%2e/secret",
            "/api/%2E%2E/secret",
            "/api/.%2e/secret",
            "/api/%2e%2e%2fsecret",
            "/api/..%5csecret",
            "/api/..\\secret",
            "/api/%252e%252e/secret",
            "/api/..;/secret",
            "/api/..%00/x",
            "/api/..%20/x",
            "/api/%ff",
        ] {
            assert!(escapes_api(path), "{path}");
        }
    }

    #[test]
    fn ordinary_paths_pass() {
        for path in [
            "/api/v1/SignIn/info",
            "/api/v1/WorkRecord/id/range/2026-10-30..2026-10-31",
            "/api/v1/a..b",
        ] {
            assert!(!escapes_api(path), "{path}");
        }
    }

    #[test]
    fn hop_by_hop_headers_include_those_named_in_connection() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONNECTION,
            HeaderValue::from_static("close, X-Secret"),
        );
        headers.insert("x-secret", HeaderValue::from_static("1"));
        headers.insert(header::TE, HeaderValue::from_static("trailers"));
        headers.insert("proxy-connection", HeaderValue::from_static("keep-alive"));
        headers.insert("x-kept", HeaderValue::from_static("1"));

        let kept: Vec<&str> = end_to_end(&headers)
            .map(|(name, _)| name.as_str())
            .collect();

        assert_eq!(kept, ["x-kept"]);
    }
}
