use std::rc::Rc;

use dioxus::core::Task;
use dioxus::prelude::*;
use futures_util::StreamExt;
use rolx_core::session::Session;

use crate::components::button::{Button, ButtonVariant};
use crate::platform::{LoginMethod, LoginPrompt, PlatformHandle};
use crate::state::{AppState, RouteEvent};

const BUTTON_ELEMENT: &str = "login-button";

/// Saves the session of a finished login and goes on to the day; or shows why it failed.
fn finish(
    platform: &PlatformHandle,
    mut state: AppState,
    mut phase: Signal<LoginPhase>,
    result: Result<Session, String>,
) {
    let result = result.and_then(|session| {
        platform
            .session_store()
            .save(&session)
            .map(|()| session)
            .map_err(|_| "The session could not be saved.".to_string())
    });
    match result {
        Ok(session) => {
            state.session.set(Some(session));
            state.navigate(RouteEvent::SignedIn);
        }
        Err(error) => phase.set(LoginPhase::Idle { error: Some(error) }),
    }
}

/// Where a login stands, as far as the screen is concerned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginPhase {
    Idle { error: Option<String> },
    Waiting { prompt: Option<LoginPrompt> },
}

#[component]
pub fn LoginView() -> Element {
    let platform = use_context::<PlatformHandle>();
    let state = use_context::<AppState>();
    let mut phase = use_signal(|| LoginPhase::Idle { error: None });
    let mut task = use_signal(|| None::<Task>);
    let mut copy_result = use_signal(|| None::<bool>);

    // A button drawn by the platform reports from outside the app, through this channel.
    let button_results = {
        let platform = platform.clone();
        use_coroutine(
            move |mut results: UnboundedReceiver<Result<Session, String>>| {
                let platform = platform.clone();
                async move {
                    while let Some(result) = results.next().await {
                        finish(&platform, state, phase, result);
                    }
                }
            },
        )
    };

    let start = {
        let platform = platform.clone();
        move |method: LoginMethod| {
            // A click can still arrive from before the buttons were replaced.
            if !matches!(*phase.peek(), LoginPhase::Idle { .. }) {
                return;
            }
            let platform = platform.clone();
            copy_result.set(None);
            phase.set(LoginPhase::Waiting { prompt: None });
            let on_prompt = Rc::new(move |prompt| {
                let mut phase = phase;
                phase.set(LoginPhase::Waiting {
                    prompt: Some(prompt),
                });
            });
            task.set(Some(spawn(async move {
                let result = platform.login(method, on_prompt).await;
                finish(&platform, state, phase, result);
            })));
        }
    };
    let cancel = move |_| {
        if let Some(running) = task.take() {
            running.cancel();
        }
        phase.set(LoginPhase::Idle { error: None });
    };

    rsx! {
        main { class: "flex flex-col items-center gap-4 p-8 text-slate-900",
            h1 { class: "text-3xl font-bold text-sky-700", "RolX" }
            LoginStatus { phase: phase(), copy_result }
            if let LoginPhase::Idle { .. } = phase() {
                if platform.login_methods().is_empty() {
                    p { id: "no-login", "No way to sign in is configured for this installation." }
                }
                for method in platform.login_methods() {
                    if method == LoginMethod::GoogleButton {
                        div {
                            id: BUTTON_ELEMENT,
                            key: "{method:?}",
                            onmounted: {
                                let platform = platform.clone();
                                move |_| {
                                    let sender = button_results.tx();
                                    platform.mount_login_button(
                                        BUTTON_ELEMENT,
                                        Rc::new(move |result| {
                                            let _ = sender.unbounded_send(result);
                                        }),
                                    );
                                }
                            },
                        }
                    } else {
                        Button {
                            key: "{method:?}",
                            variant: ButtonVariant::Primary,
                            onclick: {
                                let mut start = start.clone();
                                move |_| start(method)
                            },
                            if method == LoginMethod::Browser {
                                "Sign in with browser"
                            } else {
                                "Sign in with a code"
                            }
                        }
                    }
                }
                Button {
                    variant: ButtonVariant::Ghost,
                    onclick: move |_| state.navigate(RouteEvent::OpenSettings),
                    "Settings"
                }
            } else {
                Button {
                    id: "cancel-login",
                    variant: ButtonVariant::Outline,
                    onclick: cancel,
                    "Cancel"
                }
            }
        }
    }
}

/// What the user has to know or do in the current phase.
#[component]
pub fn LoginStatus(phase: LoginPhase, copy_result: Signal<Option<bool>>) -> Element {
    let platform = use_context::<PlatformHandle>();
    let copy_message = match copy_result() {
        Some(true) => "Copied.",
        Some(false) => "Could not copy, please copy it by hand.",
        None => "",
    };

    match phase {
        LoginPhase::Idle { error: None } => rsx! { p { "Sign in to see your time records." } },
        LoginPhase::Idle { error: Some(error) } => rsx! {
            p { id: "login-error", class: "text-red-700", "Sign-in failed: {error}" }
        },
        LoginPhase::Waiting { prompt: None } => rsx! {
            p { id: "login-waiting", "Waiting for you to sign in…" }
        },
        LoginPhase::Waiting {
            prompt: Some(LoginPrompt::ManualUrl(url)),
        } => {
            let copy = {
                let (platform, url) = (platform.clone(), url.clone());
                move |_| copy_result.set(Some(platform.copy_text(&url)))
            };
            rsx! {
                p { "The browser could not be opened. Please open this address yourself:" }
                p { id: "login-url", class: "break-all", "{url}" }
                Button { variant: ButtonVariant::Outline, onclick: copy, "Copy address" }
                p { "{copy_message}" }
            }
        }
        LoginPhase::Waiting {
            prompt: Some(LoginPrompt::DeviceCode { uri, code }),
        } => {
            let copy = {
                let (platform, code) = (platform.clone(), code.clone());
                move |_| copy_result.set(Some(platform.copy_text(&code)))
            };
            let open = {
                let (platform, uri) = (platform.clone(), uri.clone());
                move |_| {
                    platform.open_external(&uri);
                }
            };
            rsx! {
                p { "Enter this code on the sign-in page:" }
                p { id: "device-code", class: "text-3xl font-bold", "{code}" }
                p { id: "device-url", class: "break-all", "{uri}" }
                div { class: "flex flex-row gap-2",
                    Button { id: "copy-code", variant: ButtonVariant::Outline, onclick: copy, "Copy code" }
                    Button { id: "open-browser", variant: ButtonVariant::Primary, onclick: open, "Open browser" }
                }
                p { "{copy_message}" }
            }
        }
    }
}
