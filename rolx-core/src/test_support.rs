//! Shared helpers for tests that talk HTTP: a tiny in-process server and a client for it.

use std::sync::{Arc, Mutex};

use chrono::{Duration, Utc};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::client::RolxClient;
use crate::session::{BearerToken, Session};

/// Answers requests whose method matches and whose path starts with `path_prefix`.
#[derive(Clone)]
pub struct Route {
    method: &'static str,
    path_prefix: String,
    status: u16,
    body: String,
}

impl Route {
    pub fn json(method: &'static str, path_prefix: &str, status: u16, body: Value) -> Self {
        Self {
            method,
            path_prefix: path_prefix.into(),
            status,
            body: body.to_string(),
        }
    }

    pub fn empty(method: &'static str, path_prefix: &str, status: u16) -> Self {
        Self {
            method,
            path_prefix: path_prefix.into(),
            status,
            body: String::new(),
        }
    }
}

pub struct MockServer {
    base_url: String,
    requests: Arc<Mutex<Vec<(String, String, String)>>>,
}

impl MockServer {
    /// Requests of the given method as `(path, body)`; bodies that are not JSON become `null`.
    pub fn requests(&self, method: &str) -> Vec<(String, Value)> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|(m, _, _)| m == method)
            .map(|(_, path, body)| {
                (
                    path.clone(),
                    serde_json::from_str(body).unwrap_or(Value::Null),
                )
            })
            .collect()
    }

    pub fn client(&self) -> RolxClient {
        RolxClient {
            http: reqwest::Client::new(),
            server_url: self.base_url.clone(),
            session: Session {
                bearer_token: BearerToken::from_string("token".into()),
                expires: Utc::now() + Duration::days(1),
                user_id: "u1".into(),
                user_email: "a@b.c".into(),
                user_first_name: "A".into(),
                user_last_name: "B".into(),
            },
        }
    }
}

/// Starts the server. The first matching route answers; anything else gets 404.
pub async fn start_mock(routes: Vec<Route>) -> MockServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let requests: Arc<Mutex<Vec<(String, String, String)>>> = Arc::default();

    let seen = requests.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let seen = seen.clone();
            let routes = routes.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                let (body_start, content_length) = loop {
                    let n = socket.read(&mut chunk).await.unwrap();
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buf[..pos]).to_lowercase();
                        let length = head
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length:"))
                            .map_or(0, |v| v.trim().parse::<usize>().unwrap());
                        break (pos + 4, length);
                    }
                };
                while buf.len() < body_start + content_length {
                    let n = socket.read(&mut chunk).await.unwrap();
                    buf.extend_from_slice(&chunk[..n]);
                }

                let head = String::from_utf8_lossy(&buf[..body_start]).to_string();
                let mut request_line = head.lines().next().unwrap().split(' ');
                let method = request_line.next().unwrap().to_string();
                let path = request_line.next().unwrap().to_string();
                let body = String::from_utf8_lossy(&buf[body_start..body_start + content_length])
                    .to_string();
                seen.lock()
                    .unwrap()
                    .push((method.clone(), path.clone(), body));

                let (status, body) = routes
                    .iter()
                    .find(|r| r.method == method && path.starts_with(&r.path_prefix))
                    .map_or((404, String::new()), |r| (r.status, r.body.clone()));
                let response = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            });
        }
    });

    MockServer { base_url, requests }
}
