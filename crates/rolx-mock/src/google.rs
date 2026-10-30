use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::Json;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::json;
use url::Url;

use crate::{
    AppState, DEVICE_CLIENT_DENIED, DEVICE_CLIENT_EXPIRED, DEVICE_CLIENT_SLOW_DOWN, DeviceGrant,
    MOCK_ID_TOKEN,
};

const AUTH_CODE: &str = "mock-auth-code";
const DEVICE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";

fn oauth_error(error: &str) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response()
}

fn token_response() -> Response {
    Json(json!({
        "access_token": "mock-access-token",
        "token_type": "Bearer",
        "expires_in": 3600,
        "id_token": MOCK_ID_TOKEN,
        "scope": "openid email profile",
    }))
    .into_response()
}

/// Signs the "user" in at once and redirects back like Google's consent page does.
pub async fn auth(Query(params): Query<HashMap<String, String>>) -> Response {
    let Some(mut redirect) = params
        .get("redirect_uri")
        .and_then(|uri| Url::parse(uri).ok())
    else {
        return oauth_error("invalid_request");
    };
    {
        let mut query = redirect.query_pairs_mut();
        query.append_pair("code", AUTH_CODE);
        if let Some(state) = params.get("state") {
            query.append_pair("state", state);
        }
    }
    (
        StatusCode::FOUND,
        [(header::LOCATION, redirect.to_string())],
    )
        .into_response()
}

pub async fn token(
    State(state): State<AppState>,
    Form(params): Form<HashMap<String, String>>,
) -> Response {
    match params.get("grant_type").map(String::as_str) {
        Some("authorization_code") => {
            let code_ok = params.get("code").is_some_and(|code| code == AUTH_CODE);
            if code_ok && params.contains_key("code_verifier") {
                token_response()
            } else {
                oauth_error("invalid_grant")
            }
        }
        Some(DEVICE_GRANT_TYPE) => match params.get("device_code") {
            Some(device_code) => poll_device_grant(&state, device_code),
            None => oauth_error("invalid_request"),
        },
        _ => oauth_error("unsupported_grant_type"),
    }
}

fn poll_device_grant(state: &AppState, device_code: &str) -> Response {
    let (client_id, poll) = {
        let mut grants = state.device_polls.lock().expect("no panics under the lock");
        let Some(grant) = grants.get_mut(device_code) else {
            return oauth_error("invalid_grant");
        };
        grant.polls += 1;
        (grant.client_id.clone(), grant.polls)
    };

    match client_id.as_str() {
        DEVICE_CLIENT_EXPIRED => oauth_error("expired_token"),
        DEVICE_CLIENT_DENIED => oauth_error("access_denied"),
        DEVICE_CLIENT_SLOW_DOWN if poll == 1 => oauth_error("slow_down"),
        _ if poll <= state.config.device_pending_polls => oauth_error("authorization_pending"),
        _ => token_response(),
    }
}

pub async fn device_code(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(params): Form<HashMap<String, String>>,
) -> Response {
    static NEXT_DEVICE_CODE: AtomicU64 = AtomicU64::new(1);

    let Some(client_id) = params.get("client_id") else {
        return oauth_error("invalid_request");
    };
    let device_code = format!(
        "mock-device-code-{}",
        NEXT_DEVICE_CODE.fetch_add(1, Ordering::Relaxed)
    );
    state
        .device_polls
        .lock()
        .expect("no panics under the lock")
        .insert(
            device_code.clone(),
            DeviceGrant {
                client_id: client_id.clone(),
                polls: 0,
            },
        );

    let host = headers
        .get(header::HOST)
        .and_then(|host| host.to_str().ok())
        .unwrap_or("127.0.0.1");
    // Google's spelling, not RFC 8628's `verification_uri`.
    Json(json!({
        "device_code": device_code,
        "user_code": "MOCK-CODE",
        "verification_url": format!("http://{host}/google/device"),
        "expires_in": 300,
        "interval": state.config.device_poll_interval_secs,
    }))
    .into_response()
}

pub async fn device_page() -> &'static str {
    "Mock Google device page. The device grant completes by itself."
}
