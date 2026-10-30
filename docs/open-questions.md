# Open questions, decisions and manual checks

Collected while implementing `docs/plan.md`. Open questions never block the work: a default is
applied and noted here, and all questions are asked at the end.

## Open questions

| # | Question | State |
|---|---|---|
| Q10 | Google rejects the web sign-in button: "The given origin is not allowed for the given client ID" (HTTP 403). Only `http://localhost:8080` is registered for the client in `[google_web_client]` (answer of 2026-10-31), and the rejection was seen again on that date. Google's setup guide asks for both `http://localhost` and `http://localhost:<port>`. | Open: add `http://localhost` in the Google Console and retry, or give it time if the origin was added only just now. Google's guide also advises `Referrer-Policy: no-referrer-when-downgrade` for tests on http and localhost, while the host sends `strict-origin-when-cross-origin`; try that second. The cause is not proven until then. A production origin needs adding as well. |
| Q12 | The desktop app can crash in Blitz when clicks hit a screen change (`docs/blitz-spike.md`). It reproduces within seconds under scripted clicks and was seen once at a slower pace; a crash gives no message in a release build. Patch `blitz-dom` with `[patch]`, or move `apps/desktop` to the `desktop` (WebView) feature as the plan's fallback? | Open: nothing changed yet. Recommended: try the small patch first, and move to WebView if the crash stays. |

## Answered

Answered on 2026-10-31; the defaults were confirmed.

| # | Question | Answer |
|---|---|---|
| Q1 | App display name and Android bundle id | "RolX", `ch.cudos.rolxapp` |
| Q2 | UI language | English |
| Q3 | Push the `add-dioxus` branch to GitHub? | no |
| Q4 | Scope beyond CLI parity (stamping, offline-first, bookings from `docs/user-requirements.md`) | out of scope |
| Q5 | RISC-V box: distro, libc, access (SSH?) | static musl binary, no access assumed |
| Q6 | LAN access to the headless host: own domain + TLS, Tailscale, or SSH tunnel only | localhost / SSH tunnel |
| Q7 | Android minimum SDK version | dx template default: `minSdk` 24, `targetSdk` 34 |
| Q8 | Windows bundle format | dx default: NSIS installer, per user |
| Q9 | Hackathon deadline / priority if time runs out | milestone order as in the plan |
| Q11 | Remove the components only `StylePreload` rendered? | Removed as they were really unused: `calendar`, `card`, `date_picker`, `popover`, `select`. The CSP no longer allows `'unsafe-eval'`. |

## Decisions taken

| Milestone | Decision |
|---|---|
| M0 | Toolchain pinned to stable with the wasm32 and Android targets (`rust-toolchain.toml`). |
| M0 | Android env vars (`ANDROID_HOME`, `ANDROID_NDK_HOME`, `JAVA_HOME`) set at user level; the JDK is Android Studio's bundled JBR. |
| M1 | Plan deviation, ended in M2: workspace `members` omitted `crates/*` until the directory existed, because cargo rejects the glob otherwise. |
| M1 | `ROLX_CONFIG_DIR` / `ROLX_CACHE_DIR` replace the whole `<base>/rolx` directory; an empty value counts as unset. |
| M1 | `secret-scan` also looks for the actual config/session string values of 8+ characters (shorter ones such as first names would match unrelated text), both in the default dirs and any override. The path allowlist skips only the patterns, not these values. |
| M1 | `xtask check` derives the native package list from `cargo metadata` (all but `rolxapp-web`, `rolxapp-android`), so later milestones need no list edits; only wasm packages are listed. Tests get scratch `ROLX_*_DIR`s under `target/xtask-check`. |
| M2 | Behaviour fix: `sign_in` / `extend` check the status before parsing the body, so a 401 with an empty body is `Unauthorized` / `SessionExpired`, not a JSON error. |
| M2 | Behaviour fix: `redact_json` follows the `uncensored_logs` feature as documented, instead of being off in all debug builds (`-v` printed bearer tokens there). Non-JSON bodies are not logged. |
| M2 | Config parse errors show only the file and line: toml's messages quote values, which could be a client secret. |
| M2 | `SessionStore` is synchronous and has no `Send`/`Sync` bound: file, localStorage and `dioxus-sdk-storage` are all synchronous, and Dioxus tasks are local. |
| M3 | `rolx-mock` depends on `chrono`, `clap`, `tokio`, `serde` and `url` besides axum (the plan says "axum only", meant as: no `rolx-core`). Device-grant scenarios (`slow_down`, `expired_token`, `access_denied`) are chosen by `client_id`. |
| M4 | `records -v` no longer logs response bodies (real work records) unless `uncensored_logs` is on. A failed extension of a still valid session stays an error, as before. |
| M5 | Plan deviation: `login` also takes the HTTP client, because Android needs its own TLS setup (M14) and tests need plain HTTP. `rolx_auth::http_client()` is the default: HTTPS only unless the debug-build override is set. |
| M5 | The authorization URL (state, user email) is logged only with `uncensored_logs`. |
| M6 | Plan deviation: `dx` 0.8.0-alpha.3 has no `--native` flag. The command is `dx build -p rolxapp-desktop --renderer native`; `dx bundle` is expected to take the same flag (M12). |
| M6 | `apps/desktop/Dioxus.toml` points `dx` at the Tailwind files of `rolx-ui`. `cargo xtask tailwind` runs the binary `dx` installed (v4.1.5) with the same arguments, so both give the same CSS. |
| M6 | Additions to the plan: `cargo xtask tailwind` (needed by the freshness gate) and `cargo xtask input scroll` (needed by M7). `input click` positions are physical pixels on the primary monitor. |
| M6 | The window title stayed "Dioxus App" until M8; it is "RolX" since. |
| M7 | **Blitz spike passed: `apps/desktop` stays on Dioxus Native**, with five workarounds (`docs/blitz-spike.md`). Main remaining risk: a stylesheet arriving during a hover or focus change can crash Blitz, so all are loaded at startup (`StylePreload`); a small window while they load remains and is worth reporting upstream. |
| M7 | Plan deviation: components were copied from the pinned rev by hand, because `dx components add` has no package option and its git client has no TLS here (`src/components/README.md`). The plan's `dioxus-primitives` and `dioxus-icons` pins are kept; no `[patch]` was needed. |
| M8 | Plan deviation: `tailwind.css` uses `@import "tailwindcss" source(none)`. With automatic detection the result depended on the working directory, so `dx` and `xtask` generated different CSS. |
| M8 | Routes are a plain `Route` enum in a signal, not `dioxus-router`: three screens need no URLs, it works the same on every renderer, and the transitions are a pure function that is easy to test. |
| M8 | `Platform::login` takes a callback for prompts, because `rolx-ui` must compile for wasm and so cannot use `rolx-auth`'s `AuthPrompt`. The desktop app bridges the two. An unreadable session store counts as no session. |
| M9 | Date navigation is previous, next and today, without the date picker: enough for CLI parity. A session the server rejects is deleted from the store, so the next start goes straight to Login. A failed extension of a still valid session shows an error with a retry and does not sign out. |
| M10 | `Platform` gains `copy_text` (`arboard` on desktop). Cancelling drops the running login. The sign-in error text shows the underlying error, because the user needs it to tell what failed. It can name an OAuth endpoint or quote part of an error response; the state and code values were removed from those messages. It is shown, not logged. |
| M11 | The server URL is shown read-only on desktop: it comes from `config.toml`, the file the screen points to. Log out only deletes the local session; the server has no sign-out call to make. |
| M12 | Windows bundle: NSIS installer, `dx` default (Q8), 12.9 MB without the WebView2 installer, which Blitz does not need. Not code-signed, so SmartScreen warns. Smoke test: the release app files were run directly, as running the installer would change this machine; see the checklist. The command is `dx bundle -p rolxapp-desktop --renderer native --release --out-dir dist/desktop`. |
| M13 | Plan deviation: the `dx` flags are `--android --target x86_64-linux-android`. Bundle id `ch.cudos.rolxapp` (Q1 default). In the emulator the first render can take 20 s or more; checks wait for it. |
| M14 | Plan deviation: of `dioxus-sdk-storage` only `data_directory()` is used (it resolves the app directory by JNI). The session is the shared `FileSessionStore` JSON file in `files/rolx`, because the SDK's own format is opaque, and this way a mock session can be seeded with `adb run-as`. |
| M14 | Android settings are compiled in by `build.rs`, so the device client secret ends up in the APK. Google treats such secrets as not confidential for installed apps; do not share a build made with real values beyond your team. Only the server can be changed at run time, in Settings; that ends the session (a token must not reach another server), and release builds accept only HTTPS there and for the client. |
| M15 | The browser and the clipboard are reached by JNI through `dispatch` on the main thread (`apps/android/src/system.rs`), with Java exceptions cleared so a failure cannot crash the app. `Endpoints::at_base_url` exists only in debug builds. `rolx-mock` gained `--device-pending-polls` to keep the code screen up. |
| M16 | Plan deviation: `dx bundle` gives an `.aab`, which cannot be installed directly, so the APKs come from `dx build --release`: optimized, debug-signed and **debuggable** (Gradle's debug variant), 18.5 MB for `aarch64`. So on a phone with USB debugging, `adb run-as` can read the stored session; a store build needs a real signing setup, which is out of scope here. Each architecture must be built from a clean directory (`apps/android/README.md`). The minimum SDK is the `dx` default (Q7). |
| M17 | The wasm clock is checked in the browser with Playwright: a seeded session passes `is_expired` (`now()`) and the day view shows today (`today()`), without a panic. The app shows no sign-in button until M20. The bearer token in local storage (as the plan decided) is readable by any script on the origin, so the host sends a Content-Security-Policy (M18). |
| M18 | Beyond the plan's hardening: paths that would leave `/api` are refused, upstream redirects are passed on rather than followed, and a Content-Security-Policy, `nosniff` and `no-referrer` are sent. Request bodies are buffered up to the limit, responses are streamed. The upstream client trusts bundled roots, so a private CA is not supported. The CSP allows no inline scripts (only the `dx` dev page has them, checked with a release bundle in M21). It allowed `'unsafe-eval'` until Q11 removed the components that needed it. |
| M19 | `--bundle-dir` of the webhost is optional: without it only `/api` and `/config.json` are served, as behind `dx serve`. Browser checks are committed as `apps/web/tests/smoke.py`, run by hand as they need the chain up, not by the gate. |
| M20 | Web sign-in is `LoginMethod::GoogleButton`: the shared view gives the platform an element, and all JavaScript interop stays in `apps/web`. A result reaches the app through a channel, as a browser callback runs outside Dioxus. The app waits for `/config.json` before drawing, because the login screen depends on it. The browser test replaces Google's script with a stand-in. Pages now have `Referrer-Policy: strict-origin-when-cross-origin`, as Google's button needs it; `/api` keeps `no-referrer`. A `/config.json` that does not answer in 10 s counts as no configuration. |
| M21 | Plan deviation: the image is `FROM scratch` with no CA certificates, as the host brings its own roots. `dist-web` finds pip's zig by itself, because `cargo-zigbuild` asks `python3`, which on Windows can be the store's stub. The musl targets are added to `rust-toolchain.toml`. For the check, the mock ran directly in the podman WSL distro, not in an image, as §3.4 keeps it out of containers. The real release bundle under the CSP passes all browser checks: it has no inline scripts. Beyond the plan: static answers are `Cache-Control: no-cache`, so an upgrade reaches browsers, and the host stops on SIGTERM. `dist-web` clears `dx`'s output first, as `dx` leaves old wasm files that would be packed too, and hides the build machine's home path, apart from what is listed under accepted findings. |

## Manual verification checklist

Checks that need a human, real credentials or real devices. Each entry says what to do and what
to expect.

- [ ] M5: `rolxcli login` with real Google and stage. Expect the browser to open, Google's consent page, then `rolxcli whoami` showing your name.
- [ ] M5: `rolxcli login device` with real Google and stage. Expect a URL and code in the terminal; after entering the code, `rolxcli whoami` shows your name.
- [ ] M9: desktop app against stage with your real session. Expect today's entries and a total matching `rolxcli records`. Not done by the agent, so that real records stay out of its logs and screenshots.
- [ ] M10: desktop app, both sign-in buttons with real Google and stage. Expect the browser sign-in to return to the app by itself, and the code sign-in to finish after you enter the code; both end on the day view.
- [ ] M12: run `dist/desktop/RolX_0.1.0_x64-setup.exe`. Expect a SmartScreen warning (not signed), an install without administrator rights, a start-menu entry and a desktop shortcut, and the app starting with no console window. Build it first with the command in `apps/desktop/README.md`.
- [ ] M15: Android device login with real Google and stage, on an emulator or phone. Put the server and device client into `apps/android/config.local.toml` and build. Expect the code on screen, Open browser to show Google's device page in the system browser, and the day view after entering the code. Also check Copy code by pasting.
- [ ] M16: install the `aarch64` APK on a real phone (`adb install`, or copy it and allow the install). Expect the app to start and show Login; with a build holding your settings, sign in as in M15.
- [ ] M20: web sign-in with real Google, after Q10. Also checks that Google's script works under the strict CSP, which only a stand-in has done so far. Start the chain from `apps/web/README.md` with `--google-client-id <web client id>` and the server of your choice. Expect Google's button, the sign-in, and the day view.
- [ ] M21: the static Linux host against a real server: the x86_64 tarball or container with `--server https://<rolx server>`. Expect `/api/v1/SignIn/info` to answer 200. So far the musl build has only talked to the mock; TLS to stage was checked with the native build (M18).
- [ ] M21: riscv64 container under emulation. This podman machine answers "Exec format error", as it has no riscv64 emulation; install `qemu-user-static` in it, then run the riscv64 image and expect `/config.json` and `/api/v1/SignIn/info` to respond. The binary itself was checked to be a static RISC-V ELF, and it cross-compiles with TLS.
- [ ] M21: on the real RISC-V box, unpack `dist/web/rolxapp-webhost-riscv64gc-unknown-linux-musl.tar.gz` and run it as in `apps/webhost/README.md`. Expect the web app and a working `/api` with TLS to the RolX server.

## Review findings accepted without a fix

Minor findings from the per-milestone reviews that were deliberately not fixed.

- M1: `secret-scan` has no automated test for its fail-closed paths (git failure, unreadable untracked file, unparsable config or session) or for scanning real untracked files. Those need a git repo and real files; the untracked-file detection was verified by manual probes only.
- M3: `rolx-mock` answers a reversed date range with `200 []`, while the real `DateRange` throws, and sends a weekday as `dayName`, while the real server sends a rule name. No client reads or sends these.
- M4: `DateRange::day` panics on the last date chrono can represent (year 262142), as the old CLI code did. Transport and JSON errors of `records` now carry an `HTTP error:` / `JSON error:` prefix.
- M7: the scripted GUI run (coordinates and steps) is not committed, so `docs/blitz-spike.md` cannot be repeated exactly. `cargo xtask input type` handles ASCII capitals with Shift; other characters needing Shift were not tested.
- M8: Blitz's Tab order visits the hidden controls of `StylePreload`. It comes after the views, so they are reached last, but Tab can still land on invisible controls; since Q11 only a hidden button and input remain. The apps have no keyboard requirement so far. The preload test can be fooled by a deliberate edit, such as a commented-out import.
- M9: the selected date resets to today after a trip to Settings. The 5-minute repeat of `SessionKeeper` was not observed live; its check is the tested `fresh_session`, which also runs at the start. With a near-expiry session, "Try again" keeps showing the old error until the extension has finished (30 s at most).
- M10: `LoginView`'s task logic (success to save to Day, save failure, cancel) has no automated test, only the mock E2E runs; save failure was not run, and "Open browser" and the manual URL prompt were not clicked (the prompt has an SSR test).
- M11: if deleting the session fails at logout, the user reaches Login with only a log warning, and the next start signs them in again. The reachability check treating a 200 without JSON (such as a captive portal) as unreachable has no test, nor has the "not configured" state.
- M12: a panic in the release app ends it with no message anywhere (GUI subsystem, no panic hook or log file), and Blitz crashes are a known risk (`docs/blitz-spike.md`).
- M14: Android Auto Backup is left at its default, so `files/rolx/session.json` may be included in the device backup; the `dx` manifest template is not customized yet. Settings have no way back to the compiled-in server. `data_directory()` panics without an activity, which was not seen in normal use. A new APK built with another `ROLX_SERVER_URL` keeps the stored session (a developer-only case). The server file is not written atomically.
- M15: `dispatch` panics if no activity exists, which only a tap during activity destruction could reach. A failed Open browser shows no message; the address stays on screen.
- M18: no limit on concurrent connections in the webhost; `/API/x`, `//api/x` and `/api%2fx` get the app with a 200 instead of a 404; a client abort during the body reports 413. A mid-stream upstream failure has no test.
- M20: no feedback on screen while the `POST SignIn` of the web button runs, so a second click sends a second POST and an old error stays until then.
- M21: the Windows user name stays in the web app's wasm once, as Dioxus' asset metadata embeds an absolute source path (`absolute_source_path` of the focus-trap script of `dioxus-primitives`, still there after Q11, as `Button` uses that crate), which a compiler remap does not reach. The desktop and Android builds still hold the home path in panic locations. A real concern only if the build machine's account name is sensitive.
- M22: a session has no binding to its server: `rolxcli --server <address>` or an edited `server_url` sends the stored token there. Only Settings on Android guards against this.

## Dependency audit

`cargo audit` (M22, 2026-10-30): no vulnerabilities. Warnings, all in transitive dependencies of
Dioxus and Blitz: unmaintained `paste`, `proc-macro-error`, `ttf-parser`, `zbus_names`, `zvariant`,
`zvariant_derive`; unsound `glib` 0.18 (`VariantStrIter`), which comes with `wry`/`tao` for a Linux GTK target; `cargo tree`
shows it in none of the five apps' builds that were made here. They can only be solved by updating
Dioxus.

## Things that happened during the work

So that nothing surprising stays hidden:

- In M1 a `secret-scan` check used a real session value, the user id, in an untracked probe file for a moment, to prove the scanner finds it. The file was deleted, and the output named no value.
- A reviewer in M3 ran `cargo xtask secret-scan`, which reads the real `config.toml` and session even
  with temp dirs set. It printed only "no findings". Later reviews were told not to run it.
- In M14 a check printed the stage server's URL, a `config.toml` value, into the agent's log. The host
  name is also in `docs/plan.md`. In the same check a leftover setting sent the fake mock token to
  stage once, which answered 401. No real token was involved.
- In M12 the release desktop build sent one request to real Google with the fake client id
  `mock-client`, which proved that a release ignores `ROLX_GOOGLE_BASE_URL`. Google answered
  `invalid_client`.
- `rolxcli login` and the desktop login checks opened the default browser on this machine, against
  the mock. `cargo xtask input` moved the mouse and typed on the desktop during the GUI checks.
- Stage was contacted without a token in M11, M14 and M18 for reachability. `rolxcli records` and `whoami` ran with the real session, output discarded, and may have extended the
  stored token.
- Installed at user level: `dx`, Playwright and `zig` (pip), `cargo-zigbuild`, `cargo-audit`, the two musl Rust targets;
  `dx` downloaded `tailwindcss`, `wasm-bindgen`, `esbuild` and NSIS to `~/.dx`.
- Left on disk, all ignored by git: `dist/`, `apps/android/config.local.toml` (mock values), `screenshots/`
  (mock data, except status-line crops that show only "The server is reachable.").
- The podman images `rolxapp-webhost:amd64` and `:riscv64` stay in `podman-machine-rootful`, which is
  stopped. Starting that machine printed a cgroup config error, without a visible effect.
