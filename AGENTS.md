# AGENTS.md

## What this is

`rolxapp` is a Rust workspace (edition **2024**) for building **custom client apps** for the
RolX time tracking server. A slim CLI (`rolxcli`, in `apps/cli`) exists today; TUI/desktop/mobile are planned, all in Rust
where sensible. Goals and roadmap: `README.md`. Design docs: `docs/` (`api-reference.md`, `user-requirements.md`, server/frontend auth analyses).

## Layout

- `crates/rolx-core`: portable client core (native and wasm32): session, `SessionStore`, `ensure_fresh`, `RolxClient`, `DateRange`, config, redaction, API calls. No tokio, `std::fs` or `dirs` outside the `fs` feature; take the time from `clock::now()`.
- `crates/rolx-auth`: native-only Google login: loopback and device flow, with the UI left to the app through `AuthPrompt`.
- `crates/rolx-mock`: dev/test-only mock of the RolX API and Google OAuth (lib + bin, `127.0.0.1` only, default port 8090, `cargo run -p rolx-mock`). Never a dependency of anything shipped, and no dependency on `rolx-core`. Bearer tokens carry their expiry, so a session can be seeded with `bearer_token_expiring_at` or `mock-bearer-<unix time>`.
- `crates/rolx-ui`: shared Dioxus UI, renderer-agnostic and working on Blitz: no `document::eval`. The app gets everything platform-specific through the `Platform` trait. `tailwind.css` is the input, `assets/tailwind.css` the committed output. `src/components` are copied Dioxus components with local changes. Blitz rules (`docs/blitz-spike.md`): pass icons a concrete `stroke`, and every `css_module` component must render at startup (`StylePreload`) and no stylesheet may be added later, or the app can crash.
- `apps/cli`: `rolxcli`. `apps/desktop`: `rolxapp-desktop` (Dioxus Native/Blitz), implements `rolx_ui::Platform`. `apps/android`: `rolxapp-android` (Dioxus mobile, system WebView), likewise, never built in the native gate. `xtask`: workspace tasks.

## Commands

- Run the CLI: `cargo run -- -h` (default member `apps/cli`, binary `rolxcli`).
- Gate before every commit: `cargo xtask check` (fmt, secret-scan, per-package clippy and tests, wasm clippy, Dioxus duplicate guard; see `docs/plan.md` §3.1). Do not use `--workspace` builds: Dioxus renderer features must not unify.
- Secrets check alone: `cargo xtask secret-scan` (staged diff and untracked files).
- Format: `cargo fmt --all`.
- Desktop app: `dx build -p rolxapp-desktop --renderer native` (run `dx` from the workspace root), output in `target/dx/rolxapp-desktop/debug/windows/app`. Launch it with `CARGO_MANIFEST_DIR` unset, otherwise the window stays blank.
- Desktop bundle: see `apps/desktop/README.md`.
- Android app: `dx build -p rolxapp-android --android --target x86_64-linux-android` (needs `ANDROID_HOME`, `ANDROID_NDK_HOME`, `JAVA_HOME`), APK below `target/dx/rolxapp-android/debug/android/app`. Settings are compiled in from the environment or the gitignored `apps/android/config.local.toml` (see `apps/android/README.md`, also for APKs).
- Tailwind CSS: `cargo xtask tailwind` regenerates it; `dx` does too and installs the binary on its first build. Stage the result, the gate checks freshness.
- GUI checks (unlocked, awake desktop): `cargo xtask screenshot --window <title> --out screenshots/x.png`, `cargo xtask input click --window <title> --x <x> --y <y>`, `input type <text>`, `input scroll <lines>`. `/screenshots/` is ignored.
- Fetch the pinned server checkout: `cargo xtask fetch-server` (workspace member `xtask/`).
- `ROLX_GOOGLE_BASE_URL` (debug builds only, release builds never read it) points the Google endpoints at a mock, e.g. `http://127.0.0.1:8090/google`, and allows plain HTTP for login.
- Cargo feature `uncensored_logs` (on `rolx-core`, `rolx-auth` and `rolxcli`) disables log redaction; redaction is otherwise on in every build profile.
- `ROLX_CONFIG_DIR` / `ROLX_CACHE_DIR` replace the default `<config dir>/rolx` and `<cache dir>/rolx` (holding `config.toml` and `session.json`). Tests must use them with temp dirs so the real session is never touched; `cargo xtask check` sets them for all tests.
- Do not log `reqwest` errors or serde messages: they can quote the URL with the user id, or parts of an answer. `rolxcli config ls` prints client secrets, and `whoami` / `records` print personal data: never run them against the real config or session where the output is logged.

## The server you integrate with

The RolX server and Angular frontend live in a **nested checkout at `./rolx-server`** (an
ignored git repo; see `.gitignore`): a **.NET 8** ASP.NET Core Web API on MariaDB. You won't
build it, but you must match its API. The controllers under
`rolx-server/source/RolXServer/RolXServer/**/WebApi/*Controller.cs` are the source of truth for
endpoints; for request/response shapes read the matching `WebApi/Resource/*.cs` and
`WebApi/Mapping/*.cs` rather than guessing. The checkout is pinned via
`[workspace.metadata.rolx-server]` in the root `Cargo.toml`. Brief reference: `docs/api-reference.md`.

### API conventions
- Endpoints live under `api/v1/<ControllerName minus "Controller">`.
- Dates in routes are ISO (`yyyy-MM-dd`). Ranges are `{beginDate}..{endDate}` and half-open
  `(begin..end]`; for a single day use that day as begin and the next day as end.
- Protected endpoints use `[Authorize(Policy = "ActiveUser")]`.

### Authentication
1. **JWT Bearer (interactive, used by the CLI)**
   - `GET SignIn/info` → `{ googleClientId, installationId }`.
   - `POST SignIn` with `{ "googleIdToken": ... }` → `{ user, bearerToken, expires }`
     (HS256 JWT, default lifetime 1 day). `user.id` (GUID) is used in record/user routes.
   - Send JWT as `Authorization: Bearer <bearerToken>`
   - `GET api/v1/SignIn/extend` (authorized) refreshes the token before expiry.
   - The server does not validate the Google token's `aud`, so the user's own Google OAuth
     client (configured in `config.toml`) works. Details: `docs/auth-plan.md`.
2. **API key** — header `X-API-KEY`, keys managed via admin-only `api/v1/ApiKey`. Grants the
   `Supervisor` role. Not used by the app.

### Endpoints in use / likely next
- `GET WorkRecord/{userId}/range/{begin}..{end}` — used by `records`. Users may read only
  their own records unless role is `>= Supervisor`.
- `GET User` / `GET User/{id}` — elevated roles only.
