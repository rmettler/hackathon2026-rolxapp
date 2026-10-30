use std::path::{Path, PathBuf};

use super::*;

#[test]
fn override_replaces_the_default_dir() {
    let dir = resolve_dir(Some("over".into()), Some("base".into()));
    assert_eq!(dir, Some(PathBuf::from("over")));
}

#[test]
fn missing_or_empty_override_uses_rolx_below_the_default_base() {
    let expected = Some(Path::new("base").join("rolx"));
    assert_eq!(resolve_dir(None, Some("base".into())), expected);
    assert_eq!(resolve_dir(Some("".into()), Some("base".into())), expected);
}

#[test]
fn no_dir_without_override_and_default() {
    assert_eq!(resolve_dir(None, None), None);
}

#[test]
fn parse_error_gives_the_line_and_never_the_value() {
    let text = "server_url = \"u\"\ngoogle_desktop_client = \"FAKE-VALUE\"\n";

    let line = file::parse(text).unwrap_err();
    let error = ConfigError::Parse {
        path: "config.toml".into(),
        line,
    };

    assert_eq!(line, 2);
    assert!(!error.to_string().contains("FAKE-VALUE"));
}

#[test]
fn config_file_model_accepts_absent_clients_and_unknown_keys() {
    let config: AppConfig =
        toml::from_str("server_url = \"http://127.0.0.1:1\"\nother = 1\n").unwrap();

    assert_eq!(config.server_url, "http://127.0.0.1:1");
    assert!(config.google_desktop_client.is_none());
    assert!(config.google_device_client.is_none());
}
