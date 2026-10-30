# Open questions, decisions and manual checks

Collected while implementing `docs/plan.md`. Open questions never block the work: a default is
applied and noted here, and all questions are asked at the end.

## Open questions

| # | Question | Default used |
|---|---|---|
| Q1 | App display name and Android bundle id | "RolX", `ch.cudos.rolxapp` |
| Q2 | UI language | English |
| Q3 | Push the `add-dioxus` branch to GitHub? | no |
| Q4 | Scope beyond CLI parity (stamping, offline-first, bookings from `docs/user-requirements.md`) | out of scope |
| Q5 | RISC-V box: distro, libc, access (SSH?) | static musl binary, no access assumed |
| Q6 | LAN access to the headless host: own domain + TLS, Tailscale, or SSH tunnel only | localhost / SSH tunnel |
| Q7 | Android minimum SDK version | dx template default |
| Q8 | Windows bundle format | dx default |
| Q9 | Hackathon deadline / priority if time runs out | milestone order as in the plan |

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

## Manual verification checklist

Checks that need a human, real credentials or real devices. Each entry says what to do and what
to expect.

- [ ] M5: `rolxcli login` with real Google and stage. Expect the browser to open, Google's consent page, then `rolxcli whoami` showing your name.
- [ ] M5: `rolxcli login device` with real Google and stage. Expect a URL and code in the terminal; after entering the code, `rolxcli whoami` shows your name.
- [ ] M9: desktop app against stage with your real session. Expect today's entries and a total matching `rolxcli records`. Not done by the agent, so that real records stay out of its logs and screenshots.
- [ ] M10: desktop app, both sign-in buttons with real Google and stage. Expect the browser sign-in to return to the app by itself, and the code sign-in to finish after you enter the code; both end on the day view.

## Review findings accepted without a fix

Minor findings from the per-milestone reviews that were deliberately not fixed.

- M1: `secret-scan` has no automated test for its fail-closed paths (git failure, unreadable untracked file, unparsable config or session) or for scanning real untracked files. Those need a git repo and real files; the untracked-file detection was verified by manual probes only.
- M3: `rolx-mock` answers a reversed date range with `200 []`, while the real `DateRange` throws, and sends a weekday as `dayName`, while the real server sends a rule name. No client reads or sends these.
- M4: `DateRange::day` panics on the last date chrono can represent (year 262142), as the old CLI code did. Transport and JSON errors of `records` now carry an `HTTP error:` / `JSON error:` prefix.
- M7: the scripted GUI run (coordinates and steps) is not committed, so `docs/blitz-spike.md` cannot be repeated exactly. `cargo xtask input type` handles ASCII capitals with Shift; other characters needing Shift were not tested.
- M8: Blitz's Tab order visits the hidden controls of `StylePreload`. It comes after the views, so they are reached last, but Tab can still land on invisible controls. The apps have no keyboard requirement so far. The preload test can be fooled by a deliberate edit, such as a commented-out import.
- M9: the selected date resets to today after a trip to Settings. The 5-minute repeat of `SessionKeeper` was not observed live; its check is the tested `fresh_session`, which also runs at the start. With a near-expiry session, "Try again" keeps showing the old error until the extension has finished (30 s at most).
- M10: `LoginView`'s task logic (success to save to Day, save failure, cancel) has no automated test, only the mock E2E runs; save failure was not run, and "Open browser" and the manual URL prompt were not clicked (the prompt has an SSR test).
- M11: if deleting the session fails at logout, the user reaches Login with only a log warning, and the next start signs them in again. The reachability check treating a 200 without JSON (such as a captive portal) as unreachable has no test, nor has the "not configured" state.
