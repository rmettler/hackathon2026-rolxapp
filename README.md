# Cudos Hackathon 2026 RolX Apps

Base project for the custom client apps of the [RolX time tracking server](https://dev.azure.com/MFEngineeringAG/RolX/_git/RolX") built in Rust during Cudos Hackathon 2026.

## Project goals

- Provide the time tracking functionality for daily use.
- Prefer easy to use over feature rich.
- Shall be cross-platform and support CLI, terminal UI, desktop and mobile apps.
- Serves to test how well Rust is suited for the task, so Rust is used as far as applicable.

## Development setup

- Programming languages:  
  Rust
- VCS:  
  git/github
- RolX server version:  
  The app is built against the version in `Cargo.toml` metadata.  
  Run `cargo xtask fetch-server` to clone it to `./rolx-server`.
- Lint/format:  
  cargo clippy, cargo fmt
- Apps:  
  CLI: `cargo run -- -h`  
  Desktop (Dioxus): `cargo run -p rolxapp-dioxus` — log in once via the CLI or the app's login button.  
  TUI: `cargo run -p rolxapp-tui` — day view with editing, activities/favourites, year info and monthly report; logs to `<cache dir>/rolx/tui.log`.  
  TUI keys: `Tab`/`1`-`4`/`gt`/`gT` tabs, `j`/`k` move, `Ctrl-d`/`Ctrl-u` half page, `gg`/`G` first/last, `l` edit, `o`/`n` new, `dd` delete, `[`/`]` day, `t` today, `/` filter activities, `r` refresh, `q` quit. In the edit form `l` edits a field (the activity field opens a search), `s` saves, `h`/`Esc` cancels. Text fields are vim-like: `Esc` leaves insert mode for normal mode (`h l w b e 0 $`, `x D dd`, `dw cw daw diw caw ciw`, `u`), `Enter` returns to the field list. From/To is an alternative to Duration, and overlapping entries are rejected.  
  All apps share the `rolx-core` library and need `config.toml` (`cargo run -- config path`).
