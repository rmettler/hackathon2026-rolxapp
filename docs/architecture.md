# Architecture

Client apps for the RolX time tracking server, all in Rust. They do what the CLI does: sign in, show
a day's work records, keep the session alive, sign out. `docs/plan.md` has the decisions and why
they were taken; `docs/open-questions.md` has what changed on the way and what is still open.

## The pieces

```
rolxcli          → rolx-core[fs], rolx-auth                terminal
rolxapp-desktop  → rolx-ui, rolx-core[fs], rolx-auth       Dioxus Native (Blitz), no WebView
rolxapp-android  → rolx-ui, rolx-core[fs], rolx-auth       Dioxus mobile, system WebView
rolxapp-web      → rolx-ui, rolx-core                      Dioxus web, wasm32
rolxapp-webhost  → axum, reqwest                           serves rolxapp-web, proxies /api
rolx-mock        → axum                                    dev and tests only
```

| Crate | Holds | Limits |
|---|---|---|
| `crates/rolx-core` | session and its expiry, `SessionStore`, `ensure_fresh`, `DateRange`, API calls, config model | compiles for wasm32 without its `fs` feature: no tokio, no `std::fs` outside `fs` |
| `crates/rolx-auth` | Google loopback and device flows; the UI is left to the app through `AuthPrompt` | native only |
| `crates/rolx-ui` | the screens, `Platform` trait, state, copied Dioxus components, Tailwind CSS | works on every renderer; own code has no JavaScript |
| `crates/rolx-mock` | mock of the RolX API and Google's OAuth endpoints | loopback only, never shipped, no dependency on `rolx-core` |

## How an app is put together

`rolx_ui::App` takes a `PlatformHandle`. Everything that differs per platform goes through the
`Platform` trait:

| | Desktop | Android | Web |
|---|---|---|---|
| Session | `session.json` in the cache dir | `session.json` in the app's private files | browser local storage |
| Server | `config.toml` | compiled in, can be changed in Settings | the page's own origin |
| Sign-in | browser (loopback) or code | code, page opens in the system browser | Google's button |
| TLS | rustls, platform roots | rustls, bundled roots | the browser's |
| Build | `dx build --renderer native` | `dx build --android` | `dx bundle --web` |

Screens are a plain `Route` enum in a signal: Login, Day, Settings. `state::next_route` is a pure
function, and `AppState::navigate` also ends the session on sign-out or rejection.

A request goes like this: `DayView` takes the session, `ensure_fresh` extends and saves it if less
than 10 minutes are left, `get_range` fetches the half-open range. A 401 ends the session and leads
to Login; anything else shows an error with a retry. `SessionKeeper` repeats the extension every
5 minutes while the app is open.

Web, in production and in development:

```
browser → rolxapp-webhost → RolX server          production, port 8080
browser → dx serve :8080 → rolxapp-webhost :8081 → mock :8090 or RolX     development
```

`dx serve`'s proxy has no TLS, which is why the webhost sits in between in development too.

## Security properties worth keeping

- **Changing the server in Settings ends the session first** (`change_server`), so a token does not
  reach another server that way. A session has no binding to its server otherwise: `rolxcli --server
  <address>` and an edited `server_url` do send the stored token there.
- **No secrets in logs.** Errors of `reqwest` and serde are not logged, as they can quote the URL with
  the user id or parts of an answer. Redaction can only be turned off with the `uncensored_logs`
  feature. `cargo xtask secret-scan` is part of the gate.
- **The mock cannot reach a release.** `ROLX_GOOGLE_BASE_URL` and `Endpoints::at_base_url` exist only
  under `debug_assertions`, and release builds of all apps talk to the server over HTTPS only. Only
  the webhost accepts an `http://` upstream, for local setups behind it.
- **The webhost** binds loopback by default, sends no CORS headers and no cookies, ignores proxies
  from the environment, refuses paths leaving `/api`, and sends a Content-Security-Policy that allows
  neither inline scripts nor eval. A component using `document::eval` would need the latter again.

Known weak spots, on purpose: the web app's token is in local storage; the Android APKs are debuggable
and hold the device client's secret. See `docs/open-questions.md`.

## Blitz

The desktop app runs on Dioxus Native, an alpha, with a patched `blitz-dom` (`vendor/README.md`). `docs/blitz-spike.md` has the evidence and the rules:
icons need a concrete `stroke`, every `css_module` component must render at startup (`StylePreload`,
kept honest by a test), and no stylesheet may be added later, or the app can crash. If this proves
too fragile, `apps/desktop` can move to Dioxus' `desktop` (WebView) feature without touching the
screens.

## Testing

- `cargo xtask check` is the gate: fmt, secret scan, clippy and tests per package, wasm clippy, a guard
  against a second Dioxus version, fresh Tailwind CSS. Packages are built one by one, because Dioxus'
  renderer features must not unify; hence no `--workspace` builds.
- Tests run against `rolx-mock` in-process, with temp `ROLX_CONFIG_DIR` / `ROLX_CACHE_DIR`.
- `apps/web/tests/smoke.py` checks the web app in a real browser, by hand, as it needs the chain up.
- The desktop and Android apps were checked by scripted input and screenshots
  (`cargo xtask screenshot`, `cargo xtask input`, `adb`). Those scripts are not in the repo.
