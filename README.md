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
  Both share the `rolx-core` library and need `config.toml` (`cargo run -- config path`).
- Stamping (works offline, no login needed):  
  `rolxapp stamp in [ACTIVITY]`, `stamp break`, `stamp resume`, `stamp switch ACTIVITY`, `stamp out`, `stamp status`;  
  `rolxapp sync` uploads the queued entries and refreshes the activity list.  
  The desktop app has the same as buttons with a live timer, and uploads in the background after stamping.  
  Its window size and start position are the `WINDOW_*` constants at the top of `rolxapp-dioxus/src/main.rs`.
- Finding activities (many of them):  
  `rolxapp activities [WORDS…]` searches number, name, customer and project (all words must match), listing
  ★ favourites, then recently used, then the rest grouped by customer › project (`-f` / `-r` to filter).  
  The list holds **all activities still open today** (plus the ones you booked recently); `rolxapp sync` or
  `rolxapp activities --refresh` downloads it (the latter uploads nothing). The desktop app reloads it at start and
  has a "Reload list" button in the picker.  
  `stamp in|switch WORDS…` takes the same words; if they match several and you are at a terminal you get a numbered
  list. `stamp in --last` repeats your most recent activity; `rolxapp favourite WORDS… [--remove]` pins one.  
  In the desktop app, the picker offers one-click chips, search-as-you-type, ↑/↓/Enter, and a ★ to pin.
- Fixing a day:  
  `rolxapp records [DATE]` lists the entries chronologically (entries without a time of day last), numbers them
  and flags overlaps (⚠) and unbooked gaps (·); the desktop app shows the same order.  
  `rolxapp entry edit N [-d DATE] [-a ACTIVITY] [-c COMMENT] [--begin HH:MM] [--end HH:MM] [--break MINUTES] [--fix truncate|shift|break] [--force]`
  and `rolxapp entry delete N [-d DATE] [--yes]`; an edit that creates an overlap is refused unless you pick a fix or
  `--force`. `--break` changes the break while keeping start and end (the work time follows). Days before the
  server's edit lock are read-only. Queued entries: `stamp queue`, `stamp discard N`.  
  The desktop app has Edit / Delete per entry (including the break) with the same conflict choices.
- Targets:  
  A single day always has a target of 8.4 h. The week target is the sum of the server's daily nominal times
  (0 on weekends and holidays, otherwise 8.4 h × your pensum), so `rolxapp week [DATE]` and the desktop app's
  week panel show booked time, target, what is left and the pensum. The app also draws a per-day chart by activity
  that can be hidden (remembered in `gui.json`).
