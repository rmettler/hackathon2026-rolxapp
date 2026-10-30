use dioxus::prelude::*;

use crate::components::button::{Button, ButtonVariant};
use crate::state::{AppState, RouteEvent};

#[component]
pub fn DayView() -> Element {
    let state = use_context::<AppState>();
    let name = state
        .session
        .read()
        .as_ref()
        .map(|session| format!("{} {}", session.user_first_name, session.user_last_name));

    rsx! {
        main { class: "flex flex-col gap-4 p-8 text-slate-900",
            h1 { class: "text-2xl font-bold text-sky-700", "Today" }
            if let Some(name) = name {
                p { "Signed in as {name}" }
            }
            Button {
                variant: ButtonVariant::Ghost,
                onclick: move |_| state.navigate(RouteEvent::OpenSettings),
                "Settings"
            }
        }
    }
}
