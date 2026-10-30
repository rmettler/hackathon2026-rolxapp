# rolxcli

The RolX command line client, and the default package of the workspace.

```powershell
cargo run -- -h
cargo run -- login          # or: login device, for a machine without a browser
cargo run -- records 2026-10-30
cargo run -- whoami
cargo run -- logout
```

It reads `config.toml` from `%APPDATA%\rolx` (`rolxcli config path` shows where) and keeps the session
in `%LOCALAPPDATA%\rolx\session.json`. `ROLX_CONFIG_DIR` and `ROLX_CACHE_DIR` replace those
directories.

```toml
server_url = "https://<rolx server>"

[google_desktop_client]
id = "..."
secret = "..."

[google_device_client]
id = "..."
secret = "..."
```

`rolxcli config ls` prints these values, secrets included.
