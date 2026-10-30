# Dioxus components

Copied from <https://github.com/DioxusLabs/components> at rev `6b4a1488`, directory
`preview/src/components` (with `assets/dx-components-theme.css`), without docs, variants and
`component.json`. Their licence is MIT or Apache-2.0.

`dx components add` could not be used: it has no package option and assumes the current directory
is the workspace root, and its git client has no TLS support here. To update, check out a newer rev,
copy the directories again and reapply the changes below.

## Local changes, needed for Blitz (see `docs/blitz-spike.md`)

- `calendar/style.css`: flexbox layout for the day grid table.
- `date_picker/style.css`: popover anchored below the field.
- `date_picker/component.rs`: popover content aligned with `ContentAlign::Start`.
- `select/component.rs`, `date_picker/component.rs`, `calendar/component.rs`: concrete `stroke`
  colour on all icons.

Files were also run through `cargo fmt`. A component using `css_module` must render at startup
(`StylePreload` in `src/preload.rs`), or Blitz can crash.
