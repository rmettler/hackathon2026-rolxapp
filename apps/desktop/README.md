# rolxapp-desktop

The RolX desktop app, built with Dioxus Native (Blitz). See `docs/blitz-spike.md` for why, and for
the rules that keep Blitz happy.

## Run for development

From the workspace root (`dx` fails below it):

```powershell
dx build -p rolxapp-desktop --renderer native
.\target\dx\rolxapp-desktop\debug\windows\app\rolxapp-desktop.exe
```

Launch it with `CARGO_MANIFEST_DIR` unset, otherwise the window stays blank. It reads
`%APPDATA%\rolx\config.toml` and `%LOCALAPPDATA%\rolx\session.json`, the same files as `rolxcli`.

To work without real credentials, run `cargo run -p rolx-mock` in a second terminal, put this into
a `config.toml` in a temp dir:

```toml
server_url = "http://127.0.0.1:8090"

[google_desktop_client]
id = "mock-client"
secret = "mock-secret"

[google_device_client]
id = "mock-client"
secret = "mock-secret"
```

and start the app with:

```powershell
$env:ROLX_CONFIG_DIR = "<that temp dir>"
$env:ROLX_CACHE_DIR = "<that temp dir>"
$env:ROLX_GOOGLE_BASE_URL = "http://127.0.0.1:8090/google"   # debug builds only
.\target\dx\rolxapp-desktop\debug\windows\app\rolxapp-desktop.exe
```

## Release bundle

```powershell
dx bundle -p rolxapp-desktop --renderer native --release --out-dir dist/desktop
```

The installer lands in `dist/desktop`, which git ignores. `dx` downloads its packaging tool to
`~/.dx` the first time, without needing administrator rights. The bundle is not code-signed, so
Windows SmartScreen warns about it.

A release build never reads `ROLX_GOOGLE_BASE_URL`, and it talks to Google over HTTPS only.
