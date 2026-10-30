# AGENTS.md

## What this is

`rolxapp` is a Rust workspace (edition **2024**) for building **custom client apps** for the
RolX time tracking server. A slim CLI and a Dioxus desktop app exist today; TUI/mobile are planned, all in Rust
where sensible. Goals and roadmap: `README.md`. Design docs: `docs/` (`api-reference.md`, `user-requirements.md`, server/frontend auth analyses).

## Workspace layout

- `rolx-core/` — frontend-independent library: config, session, OAuth2 flows, RolX API calls
  (`rolx/`, `records.rs`, `activities.rs`), offline stamping (`stamp.rs`) and upload (`sync.rs`),
  editing a day (`dayplan.rs` pure overlap/gap analysis and fixes, `dayedit.rs` safe save), logging.
  Must not print results; frontends do the presentation.
- `.` (package `rolxapp`, `src/`) — the CLI: argument parsing (`cli.rs`) and printing (`commands/`).
- `rolxapp-dioxus/` — Dioxus 0.7 desktop app on top of `rolx-core`.
- `xtask/` — workspace tasks.

## Commands

- Run CLI: `cargo run -- -h`.
- Run desktop app: `cargo run -p rolxapp-dioxus` (heavy first build; excluded from `default-members`).
- Lint: `cargo clippy --workspace`. Format: `cargo fmt --all`. Test: `cargo test --workspace`.
- Fetch the pinned server checkout: `cargo xtask fetch-server` (workspace member `xtask/`).
- Cargo feature `uncensored_logs` disables log redaction (declared in `rolx-core`, forwarded by `rolxapp`).
- Environment: `ROLX_SERVER` overrides `server_url` (CLI `--server`, GUI); `ROLX_DATA_DIR` moves the local
  data files (`stamps.json`, `activities.json`).

## Testing safely

The session in `%LOCALAPPDATA%\rolx` belongs to a **real** RolX account, so a CLI/GUI run with default settings
changes real data. Never run `entry edit|delete`, `sync` or similar write paths without `ROLX_SERVER` pointing at a
fake server and `ROLX_DATA_DIR` at a temp folder. `cargo clippy`/`cargo test` do **not** rebuild
`target\debug\rolxapp.exe`: run `cargo build` first, and check that the binary really honours the override (e.g. point
`ROLX_SERVER` at a closed port and expect a connection error) before trusting a test run. Unit tests use the in-process
mock server in `rolx-core/src/test_support.rs`.

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
- `GET WorkRecord/{userId}/range/{begin}..{end}` — used by `records` and `sync`. Users may read only
  their own records unless role is `>= Supervisor`.
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
server are skipped). Entries without an activity stay queued until assigned. The activity cache is
`<data dir>/rolx/activities.json`. Never test uploads against a real server; `sync.tests.rs` has a mock server.
