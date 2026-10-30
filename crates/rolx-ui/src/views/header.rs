use dioxus::prelude::*;
use dioxus_icons::lucide::{LogOut, Settings};

use crate::components::button::{Button, ButtonVariant};
use crate::platform::PlatformHandle;
use crate::state::{AppState, Route, RouteEvent};

const ICON: &str = "#0369a1";

/// The bar on top of the screens other than the login: title, and as far as they apply the user,
/// settings and logout.
#[component]
pub fn Header(title: String) -> Element {
    let platform = use_context::<PlatformHandle>();
    let state = use_context::<AppState>();
    let name = state
        .session
        .read()
        .as_ref()
        .map(|session| format!("{} {}", session.user_first_name, session.user_last_name));
    let on_settings = *state.route.read() == Route::Settings;

    rsx! {
        header { class: "flex flex-row items-center gap-3 px-8 py-3 border-b border-slate-200",
            span { id: "title", class: "flex-1 text-xl font-bold text-sky-700", "{title}" }
            if let Some(name) = name.clone() {
                span { id: "user-name", "{name}" }
            }
            if !on_settings {
                Button {
                    id: "open-settings",
                    variant: ButtonVariant::Ghost,
                    "aria-label": "Settings",
                    onclick: move |_| state.navigate(RouteEvent::OpenSettings),
                    Settings { size: "18px", stroke: ICON }
                }
            }
            if name.is_some() {
                Button {
                    id: "logout",
                    variant: ButtonVariant::Ghost,
                    onclick: move |_| log_out(&platform, state),
                    LogOut { size: "18px", stroke: ICON }
                    "Log out"
                }
            }
        }
    }
}

/// Ends the session here and in the store. A store that cannot be cleaned is reported in the log
/// only: the user asked to leave, and the screen must not keep them.
pub fn log_out(platform: &PlatformHandle, state: AppState) {
    if platform.session_store().delete().is_err() {
        tracing::warn!("could not delete the session");
    }
    state.navigate(RouteEvent::SignedOut);
}
