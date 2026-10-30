# AGENTS.md

## What this is

`rolxapp` is a Rust workspace (edition **2024**) for building **custom client apps** for the
RolX time tracking server. A slim CLI and a Dioxus desktop app exist today; TUI/mobile are planned, all in Rust
where sensible. Goals and roadmap: `README.md`. Design docs: `docs/` (`api-reference.md`, `user-requirements.md`, server/frontend auth analyses).

## Workspace layout

- `rolx-core/` — frontend-independent library: config, session, OAuth2 flows, RolX API calls
  (`rolx/`, `records.rs`), logging. Must not print results; frontends do the presentation.
- `.` (package `rolxapp`, `src/`) — the CLI: argument parsing (`cli.rs`) and printing (`commands/`).
- `rolxapp-dioxus/` — Dioxus 0.7 desktop app on top of `rolx-core`.
- `xtask/` — workspace tasks.

## Commands

- Run CLI: `cargo run -- -h`.
- Run desktop app: `cargo run -p rolxapp-dioxus` (heavy first build; excluded from `default-members`).
- Lint: `cargo clippy --workspace`. Format: `cargo fmt --all`. Test: `cargo test --workspace`.
- Fetch the pinned server checkout: `cargo xtask fetch-server` (workspace member `xtask/`).
- Cargo feature `uncensored_logs` disables log redaction (declared in `rolx-core`, forwarded by `rolxapp`).

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
- `GET WorkRecord/{userId}/range/{begin}..{end}` — used by `records`. Users may read only
  their own records unless role is `>= Supervisor`.
- `GET User` / `GET User/{id}` — elevated roles only.
