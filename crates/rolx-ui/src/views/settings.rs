use dioxus::prelude::*;
use rolx_core::api::sign_in;

use crate::components::button::{Button, ButtonVariant};
use crate::components::input::Input;
use crate::platform::PlatformHandle;
use crate::state::{AppState, RouteEvent};
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

/// What came of a request to change the server.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServerChange {
    /// The session has to end, also in memory: the stored one is gone.
    pub session_ended: bool,
    pub error: Option<String>,
}

impl ServerChange {
    fn failed(error: String) -> Self {
        Self {
            session_ended: false,
            error: Some(error),
        }
    }
}

/// Switches to the server at `input`. A session belongs to the server that issued it: keeping it
/// would hand a valid token to the new address, typos included. So an actual change always ends the
/// session, wherever one might still be: an address is first checked, then the session deleted,
/// and only then the server changed. If deleting fails, the server stays. Saving the current server
/// again changes nothing.
pub fn change_server(platform: &PlatformHandle, input: &str) -> ServerChange {
    let url = match platform.validate_server_url(input) {
        Ok(url) => url,
        Err(error) => return ServerChange::failed(error),
    };
    if platform.server_url().as_deref() == Some(url.as_str()) {
        return ServerChange::default();
    }
    if platform.session_store().delete().is_err() {
        return ServerChange::failed(
            "The session could not be ended, so the server was not changed.".to_string(),
        );
    }
    ServerChange {
        session_ended: true,
        error: platform.set_server_url(&url).err(),
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
    let mut state = use_context::<AppState>();
    let server = platform
        .server_url()
        .unwrap_or_else(|| "not configured".to_string());

    let mut new_server = use_signal(String::new);
    let mut save_error = use_signal(|| None::<String>);

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
                if platform.can_change_server() {
                    div { class: "flex flex-row items-center gap-2",
                        Input {
                            id: "server-input",
                            placeholder: "https://…",
                            value: "{new_server}",
                            oninput: move |event: FormEvent| new_server.set(event.value()),
                        }
                        Button {
                            id: "save-server",
                            variant: ButtonVariant::Outline,
                            onclick: {
                                let platform = platform.clone();
                                move |_| {
                                    let input = new_server.peek().clone();
                                    let had_session = state.session.peek().is_some();
                                    let change = change_server(&platform, &input);
                                    if change.session_ended {
                                        state.session.set(None);
                                    }
                                    match change.error {
                                        Some(error) => save_error.set(Some(error)),
                                        None => {
                                            save_error.set(None);
                                            new_server.set(String::new());
                                            if had_session && change.session_ended {
                                                state.navigate(RouteEvent::SignedOut);
                                            } else {
                                                reachability.restart();
                                            }
                                        }
                                    }
                                }
                            },
                            "Save"
                        }
                    }
                    if let Some(error) = save_error() {
                        p { id: "save-error", class: "text-red-700", "{error}" }
                    }
                }
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
