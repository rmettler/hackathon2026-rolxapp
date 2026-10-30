use dioxus::prelude::*;
use rolx_core::api::sign_in;

use crate::components::button::{Button, ButtonVariant};
use crate::platform::PlatformHandle;
use crate::state::AppState;
use crate::views::Header;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reachability {
    Reachable,
    Unreachable,
    NotConfigured,
}

impl Reachability {
    fn text(self) -> &'static str {
        match self {
            Reachability::Reachable => "The server is reachable.",
            Reachability::Unreachable => "The server could not be reached.",
            Reachability::NotConfigured => "The server is not configured.",
        }
    }
}

/// Asks the one endpoint that needs no session.
pub async fn check_server(platform: &PlatformHandle) -> Reachability {
    match platform.server_url() {
        None => Reachability::NotConfigured,
        Some(url) => match sign_in::info(&platform.http_client(), &url).await {
            Ok(_) => Reachability::Reachable,
            Err(_) => Reachability::Unreachable,
        },
    }
}

#[component]
pub fn SettingsView() -> Element {
    let platform = use_context::<PlatformHandle>();
    let state = use_context::<AppState>();
    let server = platform
        .server_url()
        .unwrap_or_else(|| "not configured".to_string());

    let mut reachability = {
        let platform = platform.clone();
        use_resource(move || {
            let platform = platform.clone();
            async move { check_server(&platform).await }
        })
    };

    rsx! {
        main { class: "flex flex-col h-screen box-border text-slate-900",
            Header { title: "Settings" }
            div { class: "flex flex-col items-start gap-4 p-8",
                p { id: "server", "Server: {server}" }
                match (reachability.pending(), reachability.read().as_ref()) {
                    (false, Some(status)) => rsx! { p { id: "reachability", "{status.text()}" } },
                    _ => rsx! { p { id: "reachability", "Checking the server…" } },
                }
                Button {
                    id: "check-again",
                    variant: ButtonVariant::Outline,
                    onclick: move |_| reachability.restart(),
                    "Check again"
                }
                if let Some(location) = platform.config_location() {
                    p { id: "config-file", class: "break-all", "Config file: {location}" }
                }
                if let Some(error) = platform.config_error() {
                    p { class: "text-red-700", "{error}" }
                }
                Button {
                    id: "back",
                    variant: ButtonVariant::Ghost,
                    onclick: move |_| state.close_settings(),
                    "Back"
                }
            }
        }
    }
}
