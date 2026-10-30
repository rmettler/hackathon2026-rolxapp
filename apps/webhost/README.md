# rolxapp-webhost

Serves the RolX web app and forwards `/api/*` to the RolX server, so the browser sees one origin and
needs no CORS. It handles real bearer tokens: nothing logs a header, a body or an API path.

```powershell
cargo run -p rolxapp-webhost -- --server https://<rolx server> --bundle-dir <dir with index.html>
```

| Option | Environment | |
|---|---|---|
| `--server` | `ROLX_SERVER_URL` | where `/api/*` goes |
| `--bundle-dir` | `ROLX_BUNDLE_DIR` | the web app's files |
| `--listen` | `ROLX_LISTEN` | default `127.0.0.1:8080`; it has no TLS of its own, so keep it on loopback or put TLS in front |
| `--google-client-id` | `ROLX_GOOGLE_CLIENT_ID` | Google client of the "Web application" type, for `/config.json` |

`RUST_LOG=debug` logs each forwarded request's method and status.

Rules it keeps:

- `/api` never falls back to the app, and only the usual REST methods are forwarded.
- No CORS headers at all and no cookies, in either direction. `Origin` and client-sent
  `X-Forwarded-*` headers do not reach the upstream, nor do hop-by-hop headers.
- `Host` is the upstream's, upstream redirects are passed on rather than followed, and a proxy from
  the environment is ignored.
- Request bodies are limited to 1 MiB and 30 seconds, and paths that would leave `/api` are refused.
- Every answer carries a Content-Security-Policy, as local storage holds the bearer token. API answers
  get one that lets nothing run.

There is no limit on concurrent connections: on a LAN, put a reverse proxy in front.

TLS to the upstream trusts bundled roots, so a minimal container needs no CA files. A server with a
private CA is therefore not supported yet.
