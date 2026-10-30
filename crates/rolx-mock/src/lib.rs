//! Mock of the RolX API and the Google OAuth endpoints, for development and tests only.
//!
//! Binds to loopback only. Holds no dependency on `rolx-core`, so `rolx-core` can use it in its
//! tests. Shapes follow the server's `WebApi/Resource` and `WebApi/Mapping` classes.

mod google;
mod rolx;

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::routing::{get, post};
use chrono::Duration;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

pub use rolx::{MOCK_USER_ID, bearer_token_expiring_at};

/// The Google id token the mock issues, and whose `mock-` prefix `POST SignIn` accepts.
pub const MOCK_ID_TOKEN: &str = "mock-id-token";

/// `client_id` that makes the device grant answer `slow_down` on the first poll.
pub const DEVICE_CLIENT_SLOW_DOWN: &str = "mock-slow-down";
/// `client_id` that makes the device grant answer `expired_token`.
pub const DEVICE_CLIENT_EXPIRED: &str = "mock-expired";
/// `client_id` that makes the device grant answer `access_denied`.
pub const DEVICE_CLIENT_DENIED: &str = "mock-denied";

#[derive(Clone)]
pub struct MockConfig {
    /// Lifetime of issued bearer tokens; short or negative ones test near-expiry and expiry.
    pub token_lifetime: Duration,
    /// Poll interval the device grant asks for.
    pub device_poll_interval_secs: u64,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            token_lifetime: Duration::days(1),
            device_poll_interval_secs: 1,
        }
    }
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub config: MockConfig,
    /// Polls seen per device code.
    pub device_polls: Arc<Mutex<HashMap<String, DeviceGrant>>>,
}

pub(crate) struct DeviceGrant {
    pub client_id: String,
    pub polls: u32,
}

pub fn router(config: MockConfig) -> Router {
    let state = AppState {
        config,
        device_polls: Arc::default(),
    };
    Router::new()
        .route("/api/v1/SignIn/info", get(rolx::sign_in_info))
        .route("/api/v1/SignIn", post(rolx::sign_in))
        .route("/api/v1/SignIn/extend", get(rolx::extend))
        .route(
            "/api/v1/WorkRecord/{user_id}/range/{range}",
            get(rolx::work_record_range),
        )
        .route("/google/auth", get(google::auth))
        .route("/google/token", post(google::token))
        .route("/google/device/code", post(google::device_code))
        .route("/google/device", get(google::device_page))
        .with_state(state)
}

/// A mock serving in the background until dropped.
pub struct RunningMock {
    base_url: String,
    server: JoinHandle<std::io::Result<()>>,
}

impl RunningMock {
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Serves until the process ends.
    pub async fn wait(mut self) -> std::io::Result<()> {
        (&mut self.server)
            .await
            .unwrap_or_else(|e| Err(std::io::Error::other(e)))
    }
}

impl Drop for RunningMock {
    fn drop(&mut self) {
        self.server.abort();
    }
}

/// Starts a mock on a free loopback port.
pub async fn spawn(config: MockConfig) -> std::io::Result<RunningMock> {
    spawn_on_port(config, 0).await
}

pub async fn spawn_on_port(config: MockConfig, port: u16) -> std::io::Result<RunningMock> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
    let base_url = format!("http://{}", listener.local_addr()?);
    let app = router(config);
    let server = tokio::spawn(async move { axum::serve(listener, app).await });
    Ok(RunningMock { base_url, server })
}
