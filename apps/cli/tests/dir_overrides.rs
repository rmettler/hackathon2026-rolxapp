//! The `ROLX_CONFIG_DIR` / `ROLX_CACHE_DIR` overrides keep every later test away from the real
//! config and session, so they are checked end to end on the binary.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn rolxcli(config_dir: &Path, cache_dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rolxcli"))
        .args(args)
        .env("ROLX_CONFIG_DIR", config_dir)
        .env("ROLX_CACHE_DIR", cache_dir)
        .output()
        .expect("rolxcli should start")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

const FAKE_SESSION: &str = r#"{
    "bearer_token": "fake",
    "expires": "2000-01-01T00:00:00Z",
    "user_id": "00000000-0000-0000-0000-000000000000",
    "user_email": "test@example.invalid",
    "user_first_name": "Test",
    "user_last_name": "User"
}"#;

#[test]
fn config_path_is_below_the_config_override() {
    let config = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();

    let output = rolxcli(config.path(), cache.path(), &["config", "path"]);

    assert!(output.status.success());
    assert_eq!(
        stdout(&output),
        config.path().join("config.toml").display().to_string()
    );
}

#[test]
fn config_is_read_from_the_config_override() {
    let config = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    fs::write(
        config.path().join("config.toml"),
        "server_url = \"http://127.0.0.1:1\"\n",
    )
    .unwrap();

    let output = rolxcli(config.path(), cache.path(), &["config", "ls"]);

    assert!(output.status.success());
    assert_eq!(stdout(&output), "server_url = \"http://127.0.0.1:1\"");
}

#[test]
fn session_is_read_and_deleted_below_the_cache_override() {
    let config = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let session = cache.path().join("session.json");
    fs::write(&session, FAKE_SESSION).unwrap();

    let whoami = rolxcli(config.path(), cache.path(), &["whoami"]);
    assert!(whoami.status.success());
    assert_eq!(stdout(&whoami), "Test User <test@example.invalid>");

    let logout = rolxcli(config.path(), cache.path(), &["logout"]);
    assert!(logout.status.success());
    assert!(!session.exists());
}

#[test]
fn whoami_fails_when_the_cache_override_has_no_session() {
    let config = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();

    let output = rolxcli(config.path(), cache.path(), &["whoami"]);

    assert!(!output.status.success());
}
