# Cudos Hackathon 2026 RolX Apps

Base project for the custom client apps of the [RolX time tracking server](https://dev.azure.com/MFEngineeringAG/RolX/_git/RolX") built in Rust during Cudos Hackathon 2026.

## Project goals

- Provide the time tracking functionality for daily use.
- Prefer easy to use over feature rich.
- Shall be cross-platform and support CLI, terminal UI, desktop and mobile apps.
- Serves to test how well Rust is suited for the task, so Rust is used as far as applicable.

## The apps

| | Package | Docs |
|---|---|---|
| Command line | `rolxcli` | `apps/cli/README.md` |
| Desktop (Dioxus Native) | `rolxapp-desktop` | `apps/desktop/README.md` |
| Android (WebView) | `rolxapp-android` | `apps/android/README.md` |
| Web (wasm) | `rolxapp-web` | `apps/web/README.md` |
| Host for the web app, also for Linux and RISC-V | `rolxapp-webhost` | `apps/webhost/README.md` |

How they fit together: `docs/architecture.md`. Decisions, open questions and what still needs a human: `docs/open-questions.md`.

## Development setup

- Programming languages:  
  Rust
- VCS:  
  git/github
- RolX server version:  
  The apps are built against the version in the `[workspace.metadata.rolx-server]` section of the root `Cargo.toml`.  
  Run `cargo xtask fetch-server` to clone it to `./rolx-server`.
- Lint/format:  
  `cargo xtask check` runs the whole gate (fmt, secret scan, clippy and tests per package).
- CLI:  
  `cargo run -- -h` runs `rolxcli` from `apps/cli`.
