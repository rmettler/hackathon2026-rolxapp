# RolX apps: Dioxus desktop (Blitz), Android (WebView), Web (+ headless webhost)

Implementation plan for `D:\rolxapp-hack`. Work proceeds milestone by milestone; every milestone is
tested, then committed. Open questions never block: a default is applied, recorded in
`docs/open-questions.md` (committed), and all questions are asked at the very end.

---

## 1. Decisions (fixed)

| Topic | Decision |
|---|---|
| Dioxus | `=0.8.0-alpha.3` for the whole workspace, `dx` CLI 0.8.0-alpha.3 |
| Desktop app | Dioxus Native (Blitz), feature `native`; **experiment**, fallback = `desktop` (WebView) |
| Android app | Dioxus `mobile` (system WebView). No experiments. |
| Web app | Dioxus `web` (browser) + own Rust host `rolxapp-webhost` (static files + `/api/*` proxy) |
| Order | Phase 0 restructure → desktop → Android → web |
| CLI | package and binary `rolxcli`, lives in `apps/cli` |
| UI kit | Tailwind v4 (dx built-in), official Dioxus components from branch `0.8` @ `6b4a1488`, `dioxus-icons =0.2.0-alpha.3` (Lucide) |
| Shared UI rule | `rolx-ui` must work on Blitz (lowest common denominator); no `document::eval` in own shared code |
| Angular app | untouched; our web app runs beside it on its own origin |
| Web dev | option D: `dx serve` proxy → `rolxapp-webhost` (http, local) → RolX (https). The `dx` proxy has no TLS, hence the webhost. |
| Android login | device flow only, verification URL opened in system browser; never OAuth inside the WebView |
| Android TLS | reqwest `tls_certs_only` with bundled roots (`webpki-root-certs`), no JNI verifier init |
| Android storage | `dioxus-sdk-storage =0.8.0-alpha.3` (resolves app data dir via JNI) |
| Server pin | `[workspace.metadata.rolx-server]`, xtask reads workspace metadata |

## 2. Target structure

```
Cargo.toml                 virtual workspace, default-members = ["apps/cli"], [workspace.dependencies]
rust-toolchain.toml        stable + targets + clippy/rustfmt
crates/
  rolx-core/               portable (native + wasm32): api, session, store, config, date_range, redact
  rolx-auth/               native only (tokio): oauth2 (pkce, native_app_flow, device_flow), google, AuthPrompt
  rolx-ui/                 Dioxus, renderer-agnostic: App, Route, Platform trait, state, views, components
  rolx-mock/               dev/test only: mock RolX API + mock Google OAuth endpoints (lib + bin)
apps/
  cli/                     rolxcli
  desktop/                 rolxapp-desktop (native/Blitz)
  android/                 rolxapp-android (mobile/WebView)
  web/                     rolxapp-web (wasm32)
  webhost/                 rolxapp-webhost (axum: static + /api proxy + /config.json)
xtask/                     fetch-server, check, screenshot/input helpers, dist-web
docs/                      existing docs + open-questions.md + blitz-spike.md + architecture.md
```

Dependency graph:
```
rolxcli          → rolx-core[fs], rolx-auth
rolxapp-desktop  → rolx-ui, rolx-core[fs], rolx-auth, dioxus[native]
rolxapp-android  → rolx-ui, rolx-core, rolx-auth (device flow), dioxus[mobile], dioxus-sdk-storage
rolxapp-web      → rolx-ui, rolx-core, dioxus[web]
rolxapp-webhost  → axum, reqwest (no Dioxus)
rolx-mock        → axum only (dev-dependency of tests; runnable bin for manual/GUI runs;
                   no dependency on rolx-core: own JSON types, avoids a dev-dependency cycle)
```

## 3. Working rules

### 3.1 Definition of done per milestone
1. `cargo xtask check` passes (introduced in M1). It runs, in this order:
   - `cargo fmt --all -- --check`
   - `cargo xtask secret-scan` (staged diff and untracked files). Patterns match only real-looking
     values: JWTs (`eyJ[\w-]{10,}\.[\w-]{10,}\.`), `Bearer` followed by 20+ token characters,
     `GOCSPX-[\w-]{10,}`, `\d+-\w+\.apps\.googleusercontent\.com`; plus the actual values read at run
     time from the user's `config.toml` and session file. Placeholders like `Bearer <token>` do not
     match. Its own tests build fixtures at run time (string concatenation), and the path allowlist
     is limited to `xtask/src/secret_scan*`; docs are scanned like everything else.
   - native clippy and tests, **per package** (no `--workspace`, so Dioxus renderer features
     `native` / `mobile` / `web` never unify in one build): all packages except `rolxapp-web` and
     `rolxapp-android`: `cargo clippy -p <pkg> --all-targets -- -D warnings` and `cargo test -p <pkg>`
   - wasm clippy, **one command per package**, each run only if the package exists in
     `cargo metadata`: `rolx-core` with `--no-default-features`, `rolx-ui` and `rolxapp-web` with
     default features, all `--target wasm32-unknown-unknown -- -D warnings`
   - `rolxapp-android` is built only with `dx`/the Android target (M13+), never in the native gate
   - `cargo tree -d --workspace --target all` must list no `dioxus*` crate in two versions or from
     two sources (guards against a stray 0.7 copy; the components/`dioxus-sdk-*` git/registry
     sources are pinned to the same crates in M7 with `[patch]` if needed)
   - generated Tailwind CSS must be fresh (rebuild, `git diff --exit-code` on the CSS file) from M6
2. Milestone-specific tests (listed per milestone) pass.
3. Docs touched by the change are updated (`AGENTS.md`, `README.md`, crate READMEs).
4. `docs/open-questions.md` updated (new questions, decisions taken, deferred tests).
5. **Independent review passes** (§3.5) on the staged changes.
6. Commit.

### 3.2 VCS
- Repo is managed with plain git, branch `add-dioxus`, working tree clean at start. The leftover
  `.jj/` directory is ignored and not used.
- `docs/plan.md` is committed before M0 starts (the plan commit is amended once, at the user's
  request, to add the review rule), so no milestone commit ever contains plan edits except
  recorded deviations.
- Per milestone: stage with `git add -A` (after checking `git status` for stray files), run the
  review (§3.5), then `git commit -m "M<n>: <summary>"` (body: tests run, deferred items, review
  verdict). No pushes, no amending or rewriting of earlier milestone commits.
- `.gitignore` is extended in M0 (see M0) so build output and local files are never staged.

### 3.3 Failure policy (no interruptions)
- A test that fails for an **external** reason (needs a human Google login, missing credential,
  real device) → mark *deferred* in `docs/open-questions.md` under "Manual verification", continue.
- A test that fails for an **internal** reason → fix it; a milestone is not committed red.
- Hard blocker in a milestone → record it, skip to the next milestone that doesn't depend on it
  (dependencies in §5), come back later. If every remaining milestone depends on the blocked one
  (the chain is mostly linear), stop, write the situation and options into `docs/open-questions.md`,
  commit that, and give the final report early instead of guessing.
- Blitz spike (M7) fails a must-have without a CSS/markup workaround → switch `apps/desktop` to the
  `desktop` (WebView) feature, record decision, continue.

### 3.4 Safety and privacy
- RolX server in `config.toml` is **stage** (`rolx-stage.cudos.ch`); use it **read-only** (SignIn/info,
  SignIn/extend, WorkRecord range). No writes in this plan.
- Automated tests use `rolx-mock`; stage is only for best-effort smoke checks.
- Never print or commit tokens, client secrets, `config.toml` values, or real work records.
  Screenshots with real data are not committed; committed screenshots show mock data only.
- Tests use `ROLX_CONFIG_DIR` / `ROLX_CACHE_DIR` (introduced in **M1**, before any test runs)
  pointing to temp dirs so the real session in `%LOCALAPPDATA%\rolx\session.json` is not touched
  (except explicit stage smoke checks that reuse it read-only, plus token extension).
- `rolx-mock` is dev-only: `publish = false`, never in `default-members`, never in a bundle or
  container, no dependency on `rolx-core` (own types, to avoid a dev-dependency cycle), and the
  webhost never proxies to it unless `--server` says so.
- The `ROLX_GOOGLE_BASE_URL` override exists only under `cfg(debug_assertions)`; release builds
  ignore it (token-redirect risk).
- Webhost hardening (M18): `/api/*` never falls back to the SPA, no wildcard CORS, hop-by-hop
  headers stripped, `Host` rewritten, request body size limit, binds loopback by default.
- Dev servers bind `127.0.0.1` only (avoids Windows Firewall prompts; the Android emulator reaches the
  host loopback via `10.0.2.2`).
- No admin/UAC installs; user-scope only (rustup, cargo install, pip).

### 3.5 Independent review before every commit
Every milestone is reviewed by a **separate subagent** (fresh context, read-only) after all tests
are green and the changes are staged, and **before** the commit.

- **Reviewer:** `general` subagent, started per milestone. It gets the milestone text from this
  plan, the staged diff (`git diff --cached`), the test output summary, and the checklist below.
  It must not modify files, commit, or touch the user's session/config; it may run read-only
  commands (build, clippy, tests with temp config dirs, grep).
- **Checklist:**
  1. Does the diff implement exactly the milestone (nothing missing, no scope creep)?
  2. Correctness and edge cases (error paths, expiry/refresh, date range half-open rule, wasm
     compatibility of portable crates).
  3. Tests: meaningful, not vacuous, cover the milestone's stated cases; no test touches the real
     session or config.
  4. Security/privacy: no tokens, client secrets, `config.toml` values, real work records or
     personal data in code, logs, fixtures, screenshots or docs; Authorization never logged; dev
     servers bind `127.0.0.1` only.
  5. Consistency with repo conventions (AGENTS.md, edition 2024, existing style, comment density).
  6. Docs and `docs/open-questions.md` updated; plan deviations recorded.
  7. Stray files (build output, `.jj`, screenshots with real data) are not staged.
- **Verdict:** `APPROVE`, or findings classified `blocker` / `major` / `minor` / `nit`. Only
  blockers and majors gate the commit.
- **Handling:** blockers and majors are fixed, tests rerun, restaged, and the review repeated
  (fresh subagent, at most **3 rounds**). Rounds 2 and 3 must first verify that the earlier
  findings are fixed and only add new findings that are blockers or majors. Minors are fixed or
  recorded in `docs/open-questions.md`; nits are optional. Findings that remain after round 3 are
  recorded there and flagged in the commit body.
- **Reviewer failure:** if the subagent errors or returns no verdict, retry once; after a second
  failure record "review unavailable" in the commit body and in `docs/open-questions.md`, and
  continue (the machine checks of §3.1 still apply).
- **Binary files:** screenshots are listed by path and size only (the reviewer cannot judge pixels);
  the agent inspects them itself, and `secret-scan` plus the "mock data only" rule cover privacy.
- **Commit body** carries: tests run, deferred items, and `Review: APPROVE (round N)` plus any
  accepted minors.
- The final handover (M22) gets one extra whole-project review of the complete branch against
  this plan.

### 3.6 Test tooling
- Desktop GUI: `cargo xtask screenshot` (crate `xcap`) and `cargo xtask input` (crate `enigo`) for
  clicks/typing. Requires an unlocked, awake desktop.
- Android: emulator AVD `Medium_Phone_API_37.0` (x86_64, WHPX OK), started in the background with
  `Start-Process` (`-no-window -no-snapshot -no-audio`), `adb wait-for-device` and a
  `sys.boot_completed` poll; then `adb exec-out screencap -p`, `adb shell input`,
  `adb shell uiautomator dump`, `adb logcat`.
- Web: Playwright for Python with `channel="msedge"` (installed Edge, no browser download).
- Long-running servers (`dx serve`, `rolx-mock`, `rolxapp-webhost`, emulator) run as background
  processes and are stopped at the end of each check.
- Ports: `dx serve --web` serves on 8080 (the Google origin `http://localhost:8080` registered for
  the web client); in production the webhost's default is also `127.0.0.1:8080`. During dev the
  webhost runs on 8081 and `dx serve` proxies `/api` to it; the mock server runs on 8090.
- Screenshots are inspected by the agent (image read).

## 4. Preflight result (checked 2026-10-30) and setup

| Item | Status | Who / when |
|---|---|---|
| Rust stable 1.99.1, clippy, rustfmt | OK | – |
| target `wasm32-unknown-unknown` | missing | agent, M0 |
| targets `aarch64-linux-android`, `x86_64-linux-android` | missing | agent, M0 |
| targets `x86_64-unknown-linux-musl`, `riscv64gc-unknown-linux-musl` | missing | agent, M21 |
| `dx` 0.8.0-alpha.3 | missing | agent, M0 (`cargo install dioxus-cli --version 0.8.0-alpha.3 --locked`) |
| Android SDK (platform 37, build-tools 36, emulator, platform-tools 37, AVD API 37 x86_64, WHPX) | OK | – |
| Android NDK 30.0.16248370 | OK (installed by user, verified) | – |
| Android SDK Command-line Tools (`cmdline-tools\latest`) | OK (installed by user, verified) | – |
| JDK: Android Studio JBR (OpenJDK 25); dx uses Gradle 9.1 (supports JDK 25) | OK | – |
| `ANDROID_HOME`, `ANDROID_NDK_HOME`, `JAVA_HOME` | not set | agent, M0 (user-scope env vars) |
| Node 24, Python 3.13 | OK | Playwright: agent, M0 (`pip install --user playwright`) |
| Edge, Chrome, WebView2 156 | OK | – |
| GPU AMD Radeon 780M (wgpu/Blitz) | OK | – |
| podman 6.1.4 (machine stopped), `docker` shim → podman | OK | agent starts machine in M21 |
| zig + cargo-zigbuild | missing | agent, M21 (`pip install --user ziglang`, `cargo install cargo-zigbuild`) |
| git (branch `add-dioxus`, working tree clean, user configured) | OK | – |
| RolX stage reachable, `SignIn/info` 200 | OK | – |
| RolX source (Azure DevOps) non-interactive access | OK | `cargo xtask fetch-server` in M1 |
| `config.toml` with `server_url`, `google_desktop_client`, `google_device_client` | OK | – |
| CLI session | valid until 2026-10-31 12:31 UTC | GUI app extends; if it lapses, stage checks are deferred |
| Google OAuth client type "Web application" (origin `http://localhost:8080`) | OK (`[google_web_client]` present in `config.toml`, verified) | – |
| Network: crates.io, GitHub, dl.google.com, Google, ziglang.org | OK | – |
| Disk: D: 272 GB free | OK | – |

## 5. Milestones

Notation: **Tests** = automated (gate), **Checks** = agent-performed run/visual verification,
**Deferred** = needs a human, goes to the checklist.

### M0 – Preflight and scaffolding
- Install: rust targets (wasm32, aarch64/x86_64-linux-android), `dx` 0.8.0-alpha.3, Playwright (pip --user).
- Set user env vars `ANDROID_HOME`, `ANDROID_NDK_HOME` (newest `ndk\*`), `JAVA_HOME` (Android Studio `jbr`).
- Add `rust-toolchain.toml` (channel stable, components clippy/rustfmt, targets above).
- Create `docs/open-questions.md` (sections: Open questions, Decisions taken, Manual verification checklist).
- Extend `.gitignore`: `.jj/`, `dist/`, `/apps/*/dist`, `node_modules/`, `__pycache__/`,
  `playwright-report/`, `test-results/`, `*.local.toml`, `apps/android/config.local.toml`,
  Android/Gradle build output (`**/gen/`, `**/.gradle/`, `**/build/` under `apps/android`),
  `/screenshots/` (scratch captures; only mock-data screenshots under `docs/` are committed).
  `/target` and `/rolx-server` stay.
- **Checks:** `dx --version` = 0.8.0-alpha.3; `java -version`; NDK dir exists; emulator boots headless
  (started with `Start-Process emulator -avd Medium_Phone_API_37.0 -no-window -no-snapshot -no-audio`,
  `adb wait-for-device`, poll `getprop sys.boot_completed` = 1) then `adb emu kill`; Playwright opens
  `about:blank` in Edge; a test screen capture is readable; `git status` shows no `.jj` files.
- Commit: `M0: toolchain pin and open-questions log`.

### Phase 0 – Restructure (CLI parity, no behaviour change)

### M1 – Virtual workspace, `rolxcli`, `xtask check`
- Root `Cargo.toml` → virtual workspace (`members = ["crates/*", "apps/*", "xtask"]`,
  `default-members = ["apps/cli"]`, `resolver = "3"`, `[workspace.package]`, `[workspace.dependencies]`).
- Move `src/` → `apps/cli/src`, package/binary `rolxcli`, clap name and help texts updated.
- Server pin → `[workspace.metadata.rolx-server]`; adapt `xtask/src/util.rs` + `fetch_server.rs`.
- `ROLX_CONFIG_DIR` / `ROLX_CACHE_DIR` override the default config/cache dirs (documented in
  `AGENTS.md`); needed by every later test so the real session is never touched.
- New `cargo xtask check` (gate in §3.1, packages that exist at this point; later milestones add
  their packages to its list) and `cargo xtask secret-scan`. Fix pre-existing clippy warnings if any.
- Update `AGENTS.md`, `README.md`.
- **Tests:** `cargo xtask check`; existing pkce tests; secret-scan unit tests (detects a fake JWT,
  ignores clean text).
- **Checks:** `cargo run -- -h` shows `rolxcli`; `cargo xtask fetch-server` clones `./rolx-server` at
  the pinned rev; `rolxcli whoami` and `rolxcli records` against stage (best effort; these use the
  real session read-only, `records` may extend the token as the CLI always did).
- Commit: `M1: virtual workspace, CLI renamed to rolxcli, xtask check`.

### M2 – `rolx-core`: session, store, config, sign-in
- Move `session.rs` (no fs), `SessionStore` trait, `FileSessionStore` (feature `fs`, default on),
  `config.rs` (model + file loading behind `fs`), `redact`, `api/sign_in.rs`; thiserror errors.
- Feature `uncensored_logs` defined in `rolx-core`/`rolx-auth`, forwarded by apps.
- Verify `Approval`/`User` fields against `rolx-server/.../Auth/WebApi/Resource/*.cs`.
- **Portability rules for `rolx-core` (must compile for `wasm32-unknown-unknown` without `fs`):**
  - no `tokio`, no `std::fs`/`dirs` outside the `fs` feature, no `rand`/`sha2`/PKCE (those live in
    `rolx-auth`, native only);
  - `reqwest` declared per target: native with `rustls` (+ `json`), wasm32 with `json` only (the
    `rustls` feature does not apply to wasm);
  - `chrono` with `serde` and `wasmbind`/`js-sys` so `Utc::now()` works in the browser; a
    wasm-compatible clock helper (`now()`) is used instead of calling `Utc::now()` all over.
- **Tests:** expiry/near-expiry logic; `FileSessionStore` round trip + delete in temp dir;
  `Approval` JSON fixture → `Session`; wasm gate `cargo check -p rolx-core --no-default-features --target wasm32-unknown-unknown`.
- **Checks:** CLI commands unchanged (`whoami`, `config ls`, `config path`).
- Commit: `M2: extract rolx-core (session, store, config, sign-in)`.

### M3 – `rolx-mock`
- axum lib + bin, binds `127.0.0.1:<port>` (`--port`, default 8090):
  - RolX: `GET SignIn/info`, `POST SignIn` (accepts id tokens starting with `mock-`),
    `GET SignIn/extend` (401 on unknown/expired), `GET WorkRecord/{userId}/range/{begin}..{end}`
    (deterministic fixtures per date; 401 without bearer, 403 on foreign user id).
  - Google: `/google/auth` (302 to `redirect_uri?code&state`), `/google/token` (auth code and device
    grants), `/google/device/code`; device grant returns `authorization_pending` twice, then an id token.
  - Configurable token lifetime to test near-expiry/extension.
- Shapes taken from `rolx-server` Resource/Mapping classes.
- **Tests:** in-process integration tests of `rolx-core::api::sign_in` (sign in, extend, 401 path).
- Commit: `M3: mock RolX and Google OAuth server for tests`.

### M4 – `rolx-core`: work records, `RolxClient`, `ensure_fresh`
- `api/work_record.rs` (`Record`, `RecordEntry`, `get_range`), `DateRange::day()` (half-open rule),
  `RolxClient { http, server_url, session }`, `ensure_fresh(&http, url, session, &dyn SessionStore)`
  with typed errors (`NoSession`, `Expired`, …). CLI `records` uses it (printing stays in CLI).
- **Tests:** range formatting; Record fixture deserialization; `ensure_fresh` against mock
  (valid → no call, near expiry → extended + saved, expired → error, 401 on extend → `Expired`).
- **Checks:** `rolxcli --server http://127.0.0.1:8090 records 2026-10-30` prints mock entries
  (with temp `ROLX_CACHE_DIR` holding a mock session); `rolxcli records` against stage.
- Commit: `M4: work record API and session refresh in rolx-core`.

### M5 – `rolx-auth`
- Move `oauth2/*`; `AuthPrompt` trait (`open_url(url) -> bool`, `show_manual_url(url)`,
  `show_device_code(uri, code)`); `login(flow, creds, endpoints, &dyn AuthPrompt) -> IdToken`;
  Google endpoints overridable via `ROLX_GOOGLE_BASE_URL`, compiled in only under
  `cfg(debug_assertions)` (a test asserts the override is ignored when the cfg is off, or the code
  path is `#[cfg]`-gated and reviewed) for mock E2E.
- CLI implements `AuthPrompt` with the current `println!` texts.
- **Tests:** pkce; device flow vs mock (pending → success, slow_down, expired);
  loopback flow E2E vs mock (a test `AuthPrompt::open_url` follows the redirect with reqwest);
  `parse_auth_response` cases.
- **Checks:** `rolxcli login` / `login device` against mock Google + mock RolX via env overrides.
- **Deferred:** `rolxcli login` and `rolxcli login device` with real Google + stage.
- Commit: `M5: extract rolx-auth with AuthPrompt`.

### Phase 1a – Blitz spike

### M6 – `rolx-ui` skeleton + `apps/desktop` (Native) hello
- `rolx-ui`: `App`, placeholder view, `tailwind.css` (`@import "tailwindcss"; @source "./src";`),
  generated CSS committed (needed by `asset!` at compile time).
- `apps/desktop`: `dioxus = { features = ["native"] }`, `Dioxus.toml`, `main.rs`.
- `xtask screenshot --window <title> --out <png>` (xcap), `xtask input click|type` (enigo).
- **Tests:** gate incl. wasm check of `rolx-ui`; `cargo tree -d --workspace --target all` shows a
  single version of every `dioxus*` crate; verify early that building `rolxapp-desktop` (native) next to the other app
  crates does not unify renderer features (gate builds per package, §3.1); generated Tailwind CSS is
  fresh (`git diff --exit-code`).
- **Checks:** `dx build -p rolxapp-desktop --native`; launch; screenshot shows styled text.
- Commit: `M6: rolx-ui skeleton and Blitz desktop shell`.

### M7 – Blitz spike screen + decision
- `dx components add select,date_picker,button,input,card --git https://github.com/DioxusLabs/components --rev 6b4a1488 --module-path src/components`
  (from `crates/rolx-ui`), then fix deps: `dioxus-primitives` git rev `6b4a1488`,
  `dioxus-icons = "=0.2.0-alpha.3"`.
- Spike screen with mock data: scrolling entry list (`overflow-y-scroll`), prev/next day buttons,
  date picker, activity select, text input, Lucide icons, hover/focus styles.
- **Must-haves:** list renders and scrolls; typing works; buttons work; select opens/selects/closes;
  date picker shows the full month and picks a date; icons visible; no crash during the interaction run.
  Escape/outside-click closing is not required on Blitz.
- **Checks:** scripted interaction (xtask input) + screenshots; findings in `docs/blitz-spike.md`
  (mock-data screenshots).
- **Decision:** pass → keep Native; fail → `desktop` feature (WebView), record D-entry.
- Commit: `M7: Blitz spike screen and decision`.

### Phase 1b – Desktop app (CLI parity)

### M8 – App shell
- `Platform` trait in `rolx-ui` (session store, config, login methods, `login()`, `http_client()`,
  `open_external(url)`), `AppState` context, routes `Login` / `Day` / `Settings`, start route from
  the stored session. Desktop platform impl: `FileSessionStore`, `config.toml`, rolx-auth.
- **Tests:** unit tests for start-route and state transitions; ssr render tests of views (dioxus-ssr).
- **Checks:** with temp dirs: no session → Login screen; mock session → Day screen (screenshots).
- Commit: `M8: app shell with routes and desktop platform`.

### M9 – Day view with data
- `ensure_fresh` before each request + 5-minute extension task (portable timer: `dioxus-sdk-time`
  `=0.8.0-alpha.3`, which uses tokio on native and gloo-timers on wasm; no tokio in `rolx-core`/`rolx-ui`);
  date navigation; per-entry rows and
  total; loading/error/empty states; 401 → Login.
- **Tests:** view-model tests (totals, duration formatting shared with CLI).
- **Checks:** vs mock (fixture entries, day switching, error state with mock stopped);
  vs stage (best effort, screenshot not committed).
- Commit: `M9: day view with records`.

### M10 – Login screen
- Browser login (waiting state, cancel) and device flow (code + URL, copy button, open browser);
  `AuthPrompt` → signals; success → save session → Day.
- **Checks:** E2E vs mock Google + mock RolX for both flows (screenshots).
- **Deferred:** both flows with real Google + stage.
- Commit: `M10: login screen with browser and device flow`.

### M11 – Header, logout, settings, polish
- Header (name, logout), Settings (server URL with reachability check via `SignIn/info`,
  config file path), consistent styling.
- **Checks:** logout deletes temp session → Login; settings shows paths; reachability OK vs mock and stage.
- Commit: `M11: header, logout and settings`.

### M12 – Desktop bundle
- `dx bundle -p rolxapp-desktop --native --release` (dx default Windows format); smoke-run the bundle.
- Docs: `apps/desktop/README.md`.
- Commit: `M12: desktop release bundle`.

### Phase 2 – Android (WebView)

### M13 – Android hello
- `apps/android`: `dioxus = { features = ["mobile"] }`, `Dioxus.toml` (name, bundle id default
  `ch.cudos.rolxapp`, see open questions), `main.rs` with `rolx_ui::App` + stub platform.
- **Checks:** `dx build -p rolxapp-android --android` (x86_64 for emulator); install + launch on
  emulator; `adb` screenshot shows the Login screen.
- Commit: `M13: Android WebView shell`.

### M14 – Android platform
- HTTP client with `tls_certs_only(webpki-root-certs)`; session store via `dioxus-sdk-storage`;
  build-time config (server URL, device client) from env vars or a gitignored
  `apps/android/config.local.toml`; Settings can override the server URL.
- **Checks:** reachability vs stage (`SignIn/info` over HTTPS = TLS check, no login needed);
  Day view vs mock at `http://10.0.2.2:8090` with a mock session.
- Commit: `M14: Android TLS, storage and config`.

### M15 – Android login and logout
- Device flow UI; open verification URL in the system browser (external-link handler); copy code; logout.
- **Checks:** E2E on emulator vs mock Google + mock RolX (adb input / uiautomator).
- **Deferred:** real Google device login on emulator/real device.
- Commit: `M15: Android device-flow login`.

### M16 – Android APK
- `dx bundle -p rolxapp-android --android` for `aarch64` (devices) and `x86_64` (emulator), debug-signed.
- Docs: `apps/android/README.md` (toolchain, env vars, emulator).
- **Deferred:** install on a real device.
- Commit: `M16: Android APK bundles`.

### Phase 3 – Web

### M17 – Web hello + `rolx-core` on wasm
- `apps/web`: `dioxus = { features = ["web"] }`; localStorage `SessionStore` (key `rolxapp.session`);
  API base = `location.origin`; settings from `/config.json` (a missing/404 `/config.json`, as under a
  bare `dx serve`, falls back to defaults without an error); wasm clock verified (a
  wasm-bindgen-test or in-browser Playwright check that `now()` does not panic).
- **Checks:** `dx serve -p rolxapp-web --web`; Playwright loads `http://localhost:8080`, Login screen screenshot.
- Commit: `M17: web shell`.

### M18 – `rolxapp-webhost`
- axum: static files with SPA fallback, `/api/*` proxy to `--server` (streamed, Authorization never
  logged), `/config.json` (`googleClientId`, …), `--listen` default `127.0.0.1:8080`, `--bundle-dir`.
  rustls provider chosen for easy cross builds (ring). Hardening as listed in §3.4 (no `/api`
  SPA fallback, no wildcard CORS, hop-by-hop headers stripped, body size limit).
- **Tests:** integration tests vs mock: proxying, status/headers pass-through, only `/api/` proxied,
  SPA fallback, `/config.json`.
- **Checks:** proxy to stage `SignIn/info` = 200.
- Commit: `M18: webhost with API proxy`.

### M19 – Web day view
- Dev chain: `dx serve` with two `[[web.proxy]]` entries (`backend = "http://127.0.0.1:8081/api/"`
  and `backend = "http://127.0.0.1:8081/config.json"`) → webhost :8081 → mock.
- **Checks:** Playwright seeds a mock session in localStorage → Day view shows fixture entries;
  day navigation; logout clears storage.
- Commit: `M19: web day view through webhost`.

### M20 – Web login (Google Identity Services)
- Load `https://accounts.google.com/gsi/client`, render button with `googleClientId` from
  `/config.json`, callback → `POST SignIn` → session.
- **Checks:** button renders (Playwright) when a client id is configured.
- **Deferred:** real login (needs the Web OAuth client, origin `http://localhost:8080`).
- Commit: `M20: web login with Google Identity Services`.

### M21 – Headless packaging
- Prerequisites on PATH: `dx`, `zig` (pip `ziglang`), `cargo-zigbuild`; documented with the
  PowerShell quoting caveats for `--rev`/`--target` values.
- `cargo xtask dist-web --target <triple>`: `dx bundle --web --release` + webhost built with
  `cargo zigbuild` for `x86_64-unknown-linux-musl` and `riscv64gc-unknown-linux-musl` → tarball.
- `apps/webhost/Containerfile` (static musl binary + bundle + CA certificates), multi-arch build
  (amd64, riscv64) with podman.
- **Checks:** amd64 container: Playwright smoke vs `http://localhost:8080` with mock behind it;
  riscv64 container under emulation: `/config.json` and `/api/v1/SignIn/info` (stage, TLS) respond.
  If riscv64 emulation is unavailable in the podman machine → deferred.
- **Deferred:** run on the real RISC-V box.
- Commit: `M21: headless webhost packaging (x86_64, riscv64)`.

### M22 – Docs and handover
- `docs/architecture.md`, READMEs per app, `AGENTS.md` (commands per app, gates, env overrides),
  consolidated `docs/open-questions.md` with the manual verification checklist.
- Whole-branch review by a separate subagent against this plan (§3.5), findings handled as usual; also `cargo audit` once.
- Commit: `M22: documentation and handover`.
- Then: final report to the user + the open questions.

### Dependencies
M1 → M2 → M3 → M4 → M5. M6 needs M1. M7 needs M6. M8 needs M4, M5, M7. M9–M12 sequential after M8.
M13 needs M8. M14–M16 sequential. M17 needs M8. M18 needs M3 (can be pulled forward if blocked).
M19 needs M17 + M18. M20 needs M19. M21 needs M18 (+ M17 for the bundle). M22 last.

## 6. Initial open questions (defaults applied; asked at the end)

| # | Question | Default used |
|---|---|---|
| Q1 | App display name and Android bundle id | "RolX", `ch.cudos.rolxapp` |
| Q2 | UI language | English |
| Q3 | Push `add-dioxus` to GitHub? | no |
| Q4 | Scope beyond CLI parity (stamping, offline-first, bookings from `user-requirements.md`) | out of scope |
| Q5 | RISC-V box: distro, libc, access (SSH?) | static musl binary, no access assumed |
| Q6 | LAN access to the headless host: own domain + TLS, Tailscale, or SSH tunnel only | localhost / SSH tunnel |
| Q7 | Android minimum SDK version | dx template default |
| Q8 | Windows bundle format | dx default |
| Q9 | Hackathon deadline / priority if time runs out | milestone order as listed |
