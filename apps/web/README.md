# rolxapp-web

The RolX web app: Dioxus `web`, compiled to wasm. Its API lives on its own origin, because
`rolxapp-webhost` forwards `/api` to the RolX server; the session is in the browser's local storage.

## Development

`dx serve` has no TLS for its proxy, so it goes through the webhost, in three terminals from the
workspace root:

```powershell
cargo run -p rolx-mock
cargo run -p rolxapp-webhost -- --server http://127.0.0.1:8090 --bundle-dir apps/web --listen 127.0.0.1:8081
dx serve -p rolxapp-web --web --port 8080 --hot-patch false
```

Then open <http://localhost:8080>. Use `--server https://<rolx server>` instead of the mock to work
against a real server. `dx serve` forwards `/api/` and `/config.json` to port 8081, see
`Dioxus.toml`. With hot-patching on, editing a file can panic the running app.

## Tests

With the chain above running, and Playwright for Python (`pip install --user playwright`; it uses
the installed Edge):

```powershell
python apps/web/tests/smoke.py
```

It uses mock data and its own browser profile. Add `--production` when the webhost serves the
bundle itself, then any console error counts.
