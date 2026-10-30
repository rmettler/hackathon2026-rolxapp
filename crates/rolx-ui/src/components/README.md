# Dioxus components

Copied from <https://github.com/DioxusLabs/components> at rev `6b4a1488`, directory
`preview/src/components` (with `assets/dx-components-theme.css`), without docs, variants and
`component.json`. Their licence is MIT or Apache-2.0. Files were run through `cargo fmt`.

Only what the screens use is kept: `button` and `input`. `dx components add` could not be used: it
has no package option and assumes the current directory is the workspace root, and its git client
has no TLS support here. To add one, copy its directory from a checkout of that rev.

Before adding a component:

- It must render at startup (`StylePreload` in `src/preload.rs`) if it uses `css_module`, or Blitz can
  crash.
- `select`, `date_picker` and `popover` call `document::eval` through `dioxus-primitives`. The web
  host's Content-Security-Policy then needs `'unsafe-eval'` again, or the web app panics on start.
- `calendar` needs the `web` feature of `dioxus-primitives` in the web app, for its clock.
- These also need the Blitz changes in `docs/blitz-spike.md`.
