# RolX CLI — Google OAuth Authentication

## Context

The RolX server requires a Google ID token to authenticate users (`POST api/v1/SignIn`). The normal path (Angular frontend) uses a Web-type OAuth client with One-Tap/popup. A close reading of the server source reveals a key fact: **`SignInService.cs` line 79 calls `GoogleJsonWebSignature.ValidateAsync(token)` with one argument — it never validates the `aud` claim.** It only checks the Google signature and that the user's hosted domain is in the whitelist (`m-f.ch`, `cudos.ch`). This means a Google ID token issued by *any* OAuth client for a user on those domains will be accepted.

Solution: the user creates their own **OAuth 2.0 client** (free Google account, ~5-minute setup, no access to the production project required). Two flows are implemented in `crates/rolx-auth/src/oauth2/`:

1. **Native app flow** (default) — authorization code + PKCE (RFC 6749/8252/9700), opens the user's browser and catches the redirect on a local loopback server.
2. **Device flow** (`rolxcli login device`) — RFC 8628 Device Authorization Grant, for headless/SSH environments; no browser redirect involved at all.

## Server & Frontend Auth Analysis

Full analysis: [server-auth-implementation-analysis.md](server-auth-implementation-analysis.md) • [frontend-auth-flow-analysis.md](frontend-auth-flow-analysis.md)

Findings from reading `./rolx-server/source/RolXServer` and `./rolx-server/source/rolx-client`:

### Server (`SignInService.cs`)
- `POST api/v1/SignIn` accepts `{ "googleIdToken": string }` and returns `{ user, bearerToken, expires }`
- Validates the Google JWT signature via `GoogleJsonWebSignature.ValidateAsync(token)` — **single argument, no `aud` check**
- Only additional checks: hosted domain in whitelist (`m-f.ch`, `cudos.ch`) and user `IsConfirmed = true`
- Bearer token is HS256 JWT, default lifetime 1 day; refreshed via `GET api/v1/SignIn/extend`
- `GET api/v1/SignIn/info` returns `{ googleClientId, installationId }` (used by the Angular app)

### Angular frontend (`google-auth.service.ts`, `auth.service.ts`)
- Uses Google Identity Services (GIS) library — popup / One-Tap mode, no redirect
- Stores the server's `bearerToken` in `localStorage` under key `currentApproval`
- Refreshes every 5 minutes via `/extend`

## File / Module Structure

```
src/
  main.rs              -- clap dispatch, load config once
  cli.rs               -- clap Arg/Command derive definitions (incl. login flow subcommand)
  config.rs            -- AppConfig: server URL + OAuth creds, loaded from ~/.config/rolx/config.toml
  error.rs             -- thiserror Error enum + Result alias
  client.rs            -- RolxClient (loads session, auto-extends near expiry)
  auth/
    mod.rs
    session.rs         -- Session struct, load/save/delete ~/.cache/rolx/session.json
    rolx_sign_in.rs    -- POST api/v1/SignIn, GET api/v1/SignIn/extend
  oauth2/
    mod.rs
    device_flow.rs     -- RFC 8628 device authorization grant → TokenResponse
    native_app_flow.rs -- OAuth 2.0 auth-code + PKCE (RFC 8252), loopback redirect server
    pkce.rs            -- S256 code verifier / challenge generation
    success_page.html  -- page served on the redirect after browser authorization
  commands/
    mod.rs
    login.rs           -- pick flow → Google id_token → sign in → persist session
    logout.rs          -- delete session file
    whoami.rs          -- print user from stored session
    records.rs         -- GET WorkRecord for a day
```

## Cargo.toml Dependencies (auth-related)

```toml
[dependencies]
tokio       = { version = "1", features = ["rt-multi-thread", "macros"] }
reqwest     = { version = "0.13", features = ["form", "json", "rustls"], default-features = false }
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
clap        = { version = "4", features = ["derive", "env"] }
dirs        = "6"
chrono      = { version = "0.4", features = ["serde"] }
toml        = "1.1"
anyhow      = "1"
thiserror   = "2"

# native app flow / PKCE:
base64      = "0.23"
open        = "5"
rand        = "0.10"
sha2        = { version = "0.11", features = ["oid"] }
urlencoding = "2"
```

## CLI Commands

```
rolxcli [--server <URL>] <SUBCOMMAND>

login [FLOW]       Run OAuth flow, store session
  FLOW:
    native-app     Native app flow with PKCE (default unless specified)
    device         Device authorization flow (headless/SSH)
logout             Delete session file
whoami             Print current user from stored session
records [DATE]     Show work entries for DATE (default: today, ISO yyyy-MM-dd)
```

The server URL comes from the config file and can be overridden with `--server`. (A `ROLX_SERVER` env var was planned but is not implemented.)

## Config File: `~/.config/rolx/config.toml`

```toml
server_url           = "https://rolx.example.com"
google_client_id     = "xxxx.apps.googleusercontent.com"
google_client_secret = "GOCSPX-..."
```

`config.rs` loads this on startup; the file must exist (a helpful error message shows the template otherwise). `--server` overrides `server_url`. The OAuth client is a regular Google Cloud credential; both flows use its `client_id`, and `client_secret` is sent to Google when present.

## Session File: `~/.cache/rolx/session.json`

```json
{
  "bearer_token": "...",
  "expires": "2026-06-13T08:42:00Z",
  "user_id": "3fa85f64-...",
  "user_email": "roger@example.com",
  "user_first_name": "Roger",
  "user_last_name": "Doe"
}
```

Created with `0o600` permissions under `$XDG_CACHE_HOME/rolx/` (re-derivable by running `login` again). Loaded at start of commands that need it; if missing, the user is told to run `login`. On login, a previous session (if any) is reused to pre-fill Google's `login_hint` with the stored email.

## Native App Flow (`native_app_flow.rs`)

OAuth 2.0 authorization code flow for native apps (RFC 8252), with PKCE S256:

1. Generate a random `state` and an S256 `code_verifier`/`code_challenge` pair.
2. Bind a TCP listener on `http://localhost:<port>` / `http://[::1]:<port>` (random free port).
3. Build the Google auth URL:
   `https://accounts.google.com/o/oauth2/v2/auth?response_type=code&client_id=…&redirect_uri=…&scope=openid&state=…&code_challenge=…&code_challenge_method=S256` (plus `login_hint` if a prior session exists).
4. Open the URL in the user's browser via the `open` crate; if that fails, print the URL instead.
5. On redirect: verify `state`, extract `code`, serve a success page.
6. Exchange at `https://oauth2.googleapis.com/token` with `grant_type=authorization_code`, `code`, `redirect_uri`, `code_verifier` (+ `client_secret` if configured) → `id_token`.

## Device Flow (`device_flow.rs`)

RFC 8628, used when no browser/loopback is practical (SSH, servers):

1. `POST https://oauth2.googleapis.com/device/code` — form body: `client_id`, `scope=openid email profile` (+ `client_secret` if configured).
2. Print the verification URL (`verification_uri_complete` preferred over `verification_uri`) + `user_code`.
3. Poll `POST https://oauth2.googleapis.com/token` every `interval` seconds (default 5):
    - `authorization_pending` → keep polling
    - `slow_down` → add 5s to interval (RFC 8628 §3.5)
    - `access_denied` / `expired_token` → error + abort
    - HTTP 200 with `id_token` → done
4. Return `id_token`.

The poll loop is wrapped in `tokio::time::timeout(expires_in)`; on timeout it reports the device code as expired.

## RolX Sign-In (`rolx/sign_in.rs`)

- `POST api/v1/SignIn` body `{"googleIdToken": id_token}` → `ApprovalResponse {user, bearerToken, expires}`; 401 → "server rejected the Google token — is your account confirmed?"
- `GET api/v1/SignIn/extend` with `Authorization: Bearer` → same shape; 401 → session expired.
- Map `ApprovalResponse` → `Session` for storage.

Server JSON is camelCase; handled via `#[serde(rename_all = "camelCase")]`.

## Token Refresh Logic (`client.rs`)

Done in `RolxClient::new`, used by commands that hit the API (e.g. `records`):

```
load session → if expires < now                     → SessionExpired (told to run login)
             → if expires < now + 10min              → call /extend, save updated session
                  → if /extend returns 401           → delete session, SessionExpired
```

`whoami` reads the stored session directly and does not refresh.

## Verification

1. Create a Google Cloud project → Desktop app OAuth credential → add to `~/.config/rolx/config.toml`
2. `cargo run -- login` — opens browser; after authorizing, prints "Logged in as <name> <last> <email>"
3. `cargo run -- login device` — prints a URL + code; after entering the code in the browser, same success message
4. `cargo run -- whoami` — should print user name/email
5. `cargo run -- records` — should print today's work entries (requires an active session and server URL)
6. `cargo run -- logout` → `cargo run -- whoami` — should say session not found
