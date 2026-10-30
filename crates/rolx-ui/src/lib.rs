//! Shared Dioxus UI of the RolX apps. Must work on every renderer, Blitz being the lowest common
//! denominator: no `document::eval` in our own code.

// Copied from the Dioxus components registry and only changed for Blitz, see `components/README.md`.
#[allow(dead_code, unused_imports, clippy::all)]
pub mod components;
pub mod platform;
mod preload;
pub mod state;
mod views;

use dioxus::prelude::*;

pub use platform::{LoginFuture, LoginMethod, LoginPrompt, Platform, PlatformHandle};
use state::{AppState, Route};

const TAILWIND: Asset = asset!("/assets/tailwind.css");
const COMPONENTS_THEME: Asset = asset!("/assets/dx-components-theme.css");

#[component]
pub fn App(platform: PlatformHandle) -> Element {
    let platform = use_context_provider(|| platform);
    let state = use_context_provider(|| {
        let (route, session) = state::start(platform.session_store().load());
        AppState {
            route: Signal::new(route),
            session: Signal::new(session),
        }
    });

    rsx! {
        document::Stylesheet { href: COMPONENTS_THEME }
        document::Stylesheet { href: TAILWIND }
        match *state.route.read() {
            Route::Login => rsx! { views::LoginView {} },
            Route::Day => rsx! { views::DayView {} },
            Route::Settings => rsx! { views::SettingsView {} },
        }
        // After the views, as Blitz's Tab order also visits hidden controls.
        preload::StylePreload {}
    }
}

#[cfg(test)]
mod tests;
