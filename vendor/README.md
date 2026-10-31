# Vendored crates

## `blitz-dom`

`blitz-dom` 0.3.0-beta.4 from crates.io (without `.cargo-ok` and `Cargo.toml.orig`), used through
`[patch.crates-io]` in the root `Cargo.toml`. Its licence is MIT or Apache-2.0; the package ships no
licence text, so `LICENSE-MIT` and `LICENSE-APACHE` come from the Blitz repository at the same commit
(see `.cargo_vcs_info.json`). The repository's own licence does not cover this directory.

The one code change is upstream commit
[`f102663b`](https://github.com/DioxusLabs/blitz/commit/f102663b) "Skip nodes dropped since the last
resolve during hit testing" (#1244), in `src/node/node.rs` and `src/layout/paint_tree.rs`. Without it,
a click during a screen change can crash the desktop app, see `docs/blitz-spike.md`. Comparing with
the crates.io package shows no other code differs.

Two checks guard it. `apps/desktop/tests/hit_test_dropped_nodes.rs` panics if the fix is missing from
this directory. `cargo xtask check` fails if `rolxapp-desktop` itself uses another `blitz-dom`, which
is what happens when a Dioxus upgrade moves to another version; the test alone would not notice, as
its dev-dependency keeps using this copy.

Drop this directory, the `[patch.crates-io]` entry, the test and that check once a Dioxus release
depends on a `blitz-dom` containing the commit.
