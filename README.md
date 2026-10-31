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
  TUI keys: `Tab`/`1`-`5`/`gt`/`gT` tabs (the fifth tab, Charts, draws a line chart of one activity's booked time over a month: `j`/`k` activity, `h`/`l` month, `m` switch between running total and daily hours), `j`/`k` move, `Ctrl-d`/`Ctrl-u` half page, `gg`/`G` first/last, `l` edit, `o`/`n` new, `dd` delete, `[`/`]` day, `t` today, `w` switch the Day tab between day and week view, `a` book an absence (Krank, Ferien or Militär) for the rest of the selected day (pick the same one again to clear it; weekends are ignored), `s` stamp in and out from any tab (a running stamp shows in the header with seconds and survives closing the TUI; stamping out opens a form with From and To filled in for the activity and comment, a stamp over midnight is split into one entry per day; `S` discards the running stamp), favourite activities are always listed on the Day tab (under the entries in the day view, as rows in the week view) so you can book on them directly, `/` filter activities, `r` refresh, `q` quit. In the week view (days as columns, activities as rows) `h`/`l` move between days, `j`/`k` between activities, `e`/`Enter` edits the selected cell (an empty cell books time on that row's activity), `[`/`]` change week. In the edit form `l` edits a field (the activity field opens a search), `s` saves, `h`/`Esc` cancels. Text fields are vim-like: `Esc` leaves insert mode for normal mode (`h l w b e 0 $`, `x D dd`, `dw cw daw diw caw ciw`, `u`), `Enter` returns to the field list. From/To is an alternative to Duration, and overlapping entries are rejected.  
  TUI offline: the TUI keeps a local copy of your records (the last 24 months and the next 2) and of activities, favourites, edit lock, balance, year info and reports in `<data dir>/rolxapp-tui/`, so it starts and shows data without a connection (balance, year and report then say "as of" when they were fetched; they are never computed locally). Entries, deletions, absences and stamp bookings made without a connection are saved as pending changes (one per day) and uploaded automatically, every 30 seconds, at startup and with `R`; days that are not in the local copy yet can't be changed offline, and favourites can only change online. The header shows `offline`, `log in (L)` when the session expired (`L` logs in again; an expired session still opens the local copy), and the number of pending changes. Before uploading a day, the TUI checks that the server still has the day as it was when you changed it: if the server already has your version it is dropped from the list, if the server refused the change (for example a closed activity) it stays as rejected with the server's message, and if the day changed on the server meanwhile it becomes a conflict. `O` lists the pending changes (`Enter` opens the day, or the conflict dialog; `d` discards after confirmation; `r` retries a rejected one). The conflict dialog shows your day next to the server's and offers merge (your additions and removals applied on top of the server's day, when they don't touch the same entries), keep mine, or keep theirs. Pending days are marked in the day view (title and new entries) and in a Sync row of the week view. The months from the edit lock on are refreshed every 5 minutes, older months are fetched once; a first start downloads in the background ("Downloading 12/26"). Only one TUI can run at a time, because they would share these files.  
  All apps share the `rolx-core` library and need `config.toml` (`cargo run -- config path`).
