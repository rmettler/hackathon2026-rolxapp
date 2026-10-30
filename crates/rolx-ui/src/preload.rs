use dioxus::prelude::*;
use time::macros::date;

use crate::components::calendar::Calendar;
use crate::components::popover::{PopoverContent, PopoverRoot};

/// Loads the stylesheets of components that are otherwise rendered late, such as those in a popover.
/// Blitz can panic when a stylesheet arrives while an element's hover or focus change is pending,
/// see `docs/blitz-spike.md`. Every `css_module` component has to render at startup, here or on
/// the first screen.
#[component]
pub fn StylePreload() -> Element {
    rsx! {
        div { class: "hidden",
            PopoverRoot {
                PopoverContent { "" }
            }
            Calendar {
                selected_date: None,
                on_date_change: move |_| {},
                view_date: date!(2026 - 01 - 01),
                on_view_change: move |_| {},
            }
        }
    }
}
