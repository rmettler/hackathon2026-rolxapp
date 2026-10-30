use dioxus::prelude::*;

use crate::components::button::{Button, ButtonVariant};
use crate::platform::PlatformHandle;
use crate::state::AppState;

#[component]
pub fn SettingsView() -> Element {
    let platform = use_context::<PlatformHandle>();
    let state = use_context::<AppState>();
    let server = platform
        .server_url()
        .unwrap_or_else(|| "not configured".to_string());

    rsx! {
        main { class: "flex flex-col gap-4 p-8 text-slate-900",
            h1 { class: "text-2xl font-bold text-sky-700", "Settings" }
            p { "Server: {server}" }
            if let Some(location) = platform.config_location() {
                p { "Config file: {location}" }
            }
            if let Some(error) = platform.config_error() {
                p { class: "text-red-700", "{error}" }
            }
            Button {
                variant: ButtonVariant::Ghost,
                onclick: move |_| state.close_settings(),
                "Back"
            }
        }
    }
}
