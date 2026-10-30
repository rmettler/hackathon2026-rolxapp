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

## Manual verification checklist

Checks that need a human, real credentials or real devices. Each entry says what to do and what
to expect.

- [ ] (filled in as milestones are completed)

## Review findings accepted without a fix

Minor findings from the per-milestone reviews that were deliberately not fixed.

- M1: `secret-scan` has no automated test for its fail-closed paths (git failure, unreadable untracked file, unparsable config or session) or for scanning real untracked files. Those need a git repo and real files; the untracked-file detection was verified by manual probes only.
- M3: `rolx-mock` answers a reversed date range with `200 []`, while the real `DateRange` throws, and sends a weekday as `dayName`, while the real server sends a rule name. No client reads or sends these.
