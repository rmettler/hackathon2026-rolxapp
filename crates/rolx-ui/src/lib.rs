//! Shared Dioxus UI of the RolX apps. Must work on every renderer, Blitz being the lowest common
//! denominator: no `document::eval` in here.

use dioxus::prelude::*;

const TAILWIND: Asset = asset!("/assets/tailwind.css");

#[component]
pub fn App() -> Element {
    rsx! {
        document::Stylesheet { href: TAILWIND }
        main { class: "flex flex-col items-center gap-2 p-8 text-slate-900",
            h1 { class: "text-3xl font-bold text-sky-700", "RolX" }
            p { class: "text-base", "Time tracking" }
        }
    }
}
