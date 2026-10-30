use dioxus::prelude::*;

use crate::components::button::{Button, ButtonVariant};
use crate::platform::{LoginMethod, PlatformHandle};
use crate::state::{AppState, RouteEvent};

#[component]
pub fn LoginView() -> Element {
    let platform = use_context::<PlatformHandle>();
    let state = use_context::<AppState>();

    rsx! {
        main { class: "flex flex-col items-center gap-4 p-8 text-slate-900",
            h1 { class: "text-3xl font-bold text-sky-700", "RolX" }
            p { "Sign in to see your time records." }
            for method in platform.login_methods() {
                Button {
                    key: "{method:?}",
                    variant: ButtonVariant::Primary,
                    match method {
                        LoginMethod::Browser => "Sign in with browser",
                        LoginMethod::Device => "Sign in with a code",
                    }
                }
            }
            Button {
                variant: ButtonVariant::Ghost,
                onclick: move |_| state.navigate(RouteEvent::OpenSettings),
                "Settings"
            }
        }
    }
}
