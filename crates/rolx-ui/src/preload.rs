use dioxus::prelude::*;
use time::macros::date;

use crate::components::button::Button;
use crate::components::calendar::Calendar;
use crate::components::card::Card;
use crate::components::date_picker::DatePicker;
use crate::components::input::Input;
use crate::components::popover::{PopoverContent, PopoverRoot};
use crate::components::select::Select;

/// Loads the stylesheets of all components, hidden, whether a screen uses them yet or not. Blitz can
/// panic when a stylesheet arrives while an element's hover or focus change is pending, see
/// `docs/blitz-spike.md`. A test makes sure no `css_module` component is left out.
#[component]
pub fn StylePreload() -> Element {
    rsx! {
        div { class: "hidden",
            Button {}
            Calendar {
                selected_date: None,
                on_date_change: move |_| {},
                view_date: date!(2026 - 01 - 01),
                on_view_change: move |_| {},
            }
            Card {}
            DatePicker {
                selected_date: None,
                on_value_change: move |_| {},
            }
            Input {}
            PopoverRoot {
                PopoverContent { "" }
            }
            Select::<u8> {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    #[test]
    fn every_css_module_component_is_preloaded() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let preload = fs::read_to_string(root.join("preload.rs")).unwrap();

        let mut checked = 0;
        for entry in fs::read_dir(root.join("components")).unwrap() {
            let dir = entry.unwrap().path();
            let Ok(component) = fs::read_to_string(dir.join("component.rs")) else {
                continue;
            };
            if component.contains("#[css_module(") {
                let name = dir.file_name().unwrap().to_string_lossy().into_owned();
                let import = format!("use crate::components::{name}::");
                assert!(preload.contains(&import), "`{name}` is not preloaded");
                checked += 1;
            }
        }
        assert!(checked > 0, "no component found, did the layout change?");
    }
}
