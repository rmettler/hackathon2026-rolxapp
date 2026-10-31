# AGENTS.md

## What this is

`rolxapp` is a Rust workspace (edition **2024**) for building **custom client apps** for the
RolX time tracking server. A slim CLI, a Dioxus desktop app and a Dioxus Android app exist today; a TUI is planned,
all in Rust where sensible. Goals and roadmap: `README.md`. Design docs: `docs/` (`api-reference.md`, `user-requirements.md`, server/frontend auth analyses).

## Workspace layout

- `rolx-core/` — frontend-independent library: config, session, OAuth2 flows, RolX API calls
  (`rolx/`, `records.rs`, `activities.rs`), offline stamping (`stamp.rs`) and upload (`sync.rs`),
  editing a day (`dayplan.rs` pure overlap/gap analysis and fixes, `dayedit.rs` safe save), the week target and
  booked-time summary (`week.rs`), logging.
  Must not print results; frontends do the presentation.
- `.` (package `rolxapp`, `src/`) — the CLI: argument parsing (`cli.rs`) and printing (`commands/`).
- `rolx-ui/` — Dioxus 0.7 components shared by the GUI apps, without a platform feature: stamp panel, activity
  picker, day view, week panel, undo line (`UndoLine`), the Lucide icons (`lucide.rs`) and their CSS (`rolx_ui::CSS`:
  the design system tokens `tokens.css`, then `style.css` built only on them), plus the hooks `use_app_state`
  (day, stamp state, activities, messages, day resource) and `use_undo`. `set_config_source` replaces the config file
  (e.g. an embedded config); `Compact` (context) asks for a narrow layout: the CSS for it is scoped under `.compact`,
  and the markup only differs where `use_compact()` is checked (picker sheet, day cards, tappable week chart). Without
  it the desktop markup and look stay unchanged. Never loads or saves `gui.json`.
- `rolxapp-dioxus/` — Dioxus 0.7 desktop app on top of `rolx-ui`: window, tray, hotkey, command palette, away
  detection, login, layout (`app.css`) and the brand font from the design system checkout (`fonts.rs`); keeps its
  settings in `gui.json` (`settings.rs`).
- `rolx-mobile/` — Dioxus 0.7 Android app on top of `rolx-ui` (`Compact(true)`, root class `compact`): Stamp, Day and
  Week tabs, login (browser or device code with QR), settings in `mobile.json`, page events in JS (`PAGE_EVENTS`:
  back in the foreground, online/offline, swipes), native features (`native.rs`: notification with buttons and as a Live
  Update, vibration, share)
  and stamp links `rolx://stamp?…` with the app shortcuts that use them (`links.rs`; received by the Java
  `StampLinkActivity` in the `android/plugin` Gradle module, which also has the icons). Embeds the config at build time (`build.rs`, passed on with
  `rolx_ui::set_config_source`) and stores session and data files in the app's files dir
  (`rolx_core::paths::set_base_dir`). `build.rs` also embeds the KMR Apparat brand font when the design system
  checkout (or `ROLX_FONT_DIR`) is there at build time. The Gradle module `android/plugin` (icons, and in Kotlin the
  home-screen widget, the WorkManager background upload and the biometric app lock) calls into Rust through
  `RolxNative`, i.e. the `Java_…` exports in `src/bridge.rs`, which may run without the activity (`ndk_context` is
  unset then: they pass the context on with `android::with_context`); the app calls out via `src/plugin.rs`. `android/AndroidManifest.xml` sets a DayNight theme (else the web view is
  never dark) and `ACCESS_NETWORK_STATE` (else `navigator.onLine` never changes).
- `xtask/` — workspace tasks.

## Commands

- Run CLI: `cargo run -- -h`.
- Run desktop app: `cargo run -p rolxapp-dioxus` (heavy first build; excluded from `default-members`).
- Lint: `cargo clippy --workspace`. Format: `cargo fmt --all`. Test: `cargo test --workspace`.
- Fetch the pinned server checkout: `cargo xtask fetch-server` (workspace member `xtask/`).
- Run mobile app (`rolx-mobile/`, Dioxus, Android): `cargo xtask android {devices,emulator,run,apk}`; see `docs/dev-setup.md`.
- Fake RolX server for end-to-end tests: `cargo xtask fake-server [--port 5080] [--bind 0.0.0.0]` (in-memory, seeded,
  `GET /` shows its state); `cargo xtask fake-session --out <path>` writes a matching `session.json`. Android:
  `cargo xtask android run --fake-server` (embeds a config for it, sets up `adb reverse`) and
  `cargo xtask android push-session`. See `docs/dev-setup.md` "Testing against the fake server".
- Fetch the Cudos design system (company style guide) to `./design-system` (ignored): `cargo xtask fetch-design-system`.
  It needs Azure DevOps credentials; the pin is `[package.metadata.design-system]` in `Cargo.toml`.
- Cargo feature `uncensored_logs` disables log redaction (declared in `rolx-core`, forwarded by `rolxapp`).
- Environment: `ROLX_SERVER` overrides `server_url` (CLI `--server`, GUI); `ROLX_DATA_DIR` moves the local
  data files (`stamps.json`, `activities.json`).

## Testing safely

The session in `%LOCALAPPDATA%\rolx` belongs to a **real** RolX account, so a CLI/GUI run with default settings
changes real data. Never run `entry edit|delete`, `sync` or similar write paths without `ROLX_SERVER` pointing at a
fake server and `ROLX_DATA_DIR` at a temp folder. `cargo clippy`/`cargo test` do **not** rebuild
`target\debug\rolxapp.exe`: run `cargo build` first, and check that the binary really honours the override (e.g. point
`ROLX_SERVER` at a closed port and expect a connection error) before trusting a test run. Unit tests use the in-process
mock server in `rolx-core/src/test_support.rs`. For the GUI, build a copy with a different exe name (WebView2 shares one
data folder per exe name), start it with `ROLX_SERVER`, `ROLX_DATA_DIR` and
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port>` to drive it over the DevTools protocol, and
set `ROLX_IDLE_SECS` (threshold) and `ROLX_FAKE_IDLE_FILE` (a file holding the idle seconds to report) to try the
away detection without sitting still.

Use `cargo xtask fake-server` as that fake server (`ROLX_SERVER=http://127.0.0.1:5080`). The CLI and GUI still read
the real session (no override); the fake server accepts its token, but back the file up first and compare its hash
afterwards, as an extend near expiry or a `login` would save a fake token over it. Never run `logout`.

## The server you integrate with

The RolX server and Angular frontend live in a **nested checkout at `./rolx-server`** (an
ignored git repo; see `.gitignore`): a **.NET 8** ASP.NET Core Web API on MariaDB. You won't
build it, but you must match its API. The controllers under
`rolx-server/source/RolXServer/RolXServer/**/WebApi/*Controller.cs` are the source of truth for
endpoints; for request/response shapes read the matching `WebApi/Resource/*.cs` and
`WebApi/Mapping/*.cs` rather than guessing. The checkout is pinned via
`[package.metadata.rolx-server]` in `Cargo.toml`. Brief reference: `docs/api-reference.md`.

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
- `GET WorkRecord/{userId}/range/{begin}..{end}` — used by `records`, `week` and `sync`. Users may read only
  their own records unless role is `>= Supervisor`. Each returned day has `nominalWorkTime` (seconds): 0 on
  weekends/holidays, otherwise the server's per-day time × the part-time factor; the week target is their sum.
- `PUT WorkRecord/{userId}/{date}` — used by `sync`. **Replaces the whole day**, so always GET the day, modify, and
  send the complete record back (`records::fetch_day_record` / `update_day_record`). Entry rules: `activityId` must
  be a valid open activity for the user, `duration` is net work time, and `begin + duration + pause` must stay
  within the day (seconds since midnight). Days before the edit lock are rejected.
- `GET Activity?unlessEndedBeforeDate=<date>` — the complete activity list (used for the cache).
  `GET Activity/suitable/{userId}/{date}` only returns what the user **booked within ±7 days**, so it is just
  added to the list for activities that may have been closed since. `GET activity/favourite` marks favourites.
  Bookable = not closed at the entry's date and billability valid for the user's position (server rejects others).
- `GET User` / `GET User/{id}` — elevated roles only.

### Offline-first stamping
`rolx-core::stamp` records sessions in `<data dir>/rolx/stamps.json` (open session + queue of finished entries)
without network or login; `rolx-core::sync` uploads the queue per day and is idempotent (entries already on the
server are skipped). Entries without an activity stay queued until assigned. Change the stamp file with
`StampState::update` (load, change, save under a process-wide lock): on Android the app, its widget and the
background upload run in one process; uploads take turns too (`sync::run`). The activity cache is
`<data dir>/rolx/activities.json`. Never test uploads against a real server; `sync.tests.rs` has a mock server.
A session open for `STALE_AFTER_HOURS` (12) counts as forgotten (`StampState::stale`); frontends must not close it
silently at "now" (CLI refuses, the app shows a banner). Day changes that go through `dayedit::save_day_tracked` are
remembered in `<data dir>/rolx/undo.json` for `undo_last`.
