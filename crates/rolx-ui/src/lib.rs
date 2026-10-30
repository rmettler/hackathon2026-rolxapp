//! Shared Dioxus UI of the RolX apps. Must work on every renderer, Blitz being the lowest common
//! denominator: no `document::eval` in our own code.

// Copied from the Dioxus components registry and only changed for Blitz, see `components/README.md`.
#[allow(dead_code, unused_imports, clippy::all)]
pub mod components;
mod preload;
mod spike;

use dioxus::prelude::*;

const TAILWIND: Asset = asset!("/assets/tailwind.css");
const COMPONENTS_THEME: Asset = asset!("/assets/dx-components-theme.css");

#[component]
pub fn App() -> Element {
    rsx! {
        document::Stylesheet { href: COMPONENTS_THEME }
        document::Stylesheet { href: TAILWIND }
        preload::StylePreload {}
        spike::SpikeScreen {}
    }
}
