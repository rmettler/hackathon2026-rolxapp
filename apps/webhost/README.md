# rolxapp-webhost

Serves the RolX web app and forwards `/api/*` to the RolX server, so the browser sees one origin and
needs no CORS. It handles real bearer tokens: nothing logs a header, a body or an API path.

```powershell
cargo run -p rolxapp-webhost -- --server https://<rolx server> --bundle-dir <dir with index.html>
```

| Option | Environment | |
|---|---|---|
| `--server` | `ROLX_SERVER_URL` | where `/api/*` goes |
| `--bundle-dir` | `ROLX_BUNDLE_DIR` | the web app's files; without it only `/api` and `/config.json` are served, as behind `dx serve` |
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

## Packaging for Linux

Needs on PATH: `dx`, `cargo-zigbuild` (`cargo install cargo-zigbuild --locked`) and zig (`pip install
--user ziglang` is enough, or a `zig` of your own), plus the Rust targets from `rust-toolchain.toml`.

```powershell
cargo xtask dist-web --target x86_64-unknown-linux-musl
cargo xtask dist-web --target riscv64gc-unknown-linux-musl
```

This builds the web app in release mode and a static host, and gives
`dist/web/rolxapp-webhost-<target>.tar.gz` with the binary and `public/`, and the same files
unpacked in `dist/web/<target>/`. In PowerShell, pass `--target` values as they are; they need no
quoting. To run it from the unpacked tarball:

```sh
./rolxapp-webhost --server https://<rolx server> --bundle-dir public --google-client-id <id>
```

### Container

The image holds only the binary and the app, about 7 MB. Build one per architecture, with the
unpacked directory as the context, and combine them:

```powershell
podman build --platform linux/amd64 -t rolxapp-webhost:amd64 -f apps/webhost/Containerfile dist/web/x86_64-unknown-linux-musl
podman build --platform linux/riscv64 -t rolxapp-webhost:riscv64 -f apps/webhost/Containerfile dist/web/riscv64gc-unknown-linux-musl
podman manifest create rolxapp-webhost:multi
podman manifest add rolxapp-webhost:multi containers-storage:localhost/rolxapp-webhost:amd64
podman manifest add rolxapp-webhost:multi containers-storage:localhost/rolxapp-webhost:riscv64
```

Inside the container it listens on all interfaces, so publish the port on loopback only:

```powershell
podman run -p 127.0.0.1:8080:8080 -e ROLX_GOOGLE_CLIENT_ID=<id> rolxapp-webhost:amd64 --server https://<rolx server>
```

The mock is not part of any image. To test against it in a podman machine on WSL, cross-build it,
run it inside the machine's distro, and give the container that network, `--network host` with
`ROLX_LISTEN=127.0.0.1:8080` and `--server http://127.0.0.1:8090`. Then
`python apps/web/tests/smoke.py http://localhost:8080 --production` checks the whole package.
