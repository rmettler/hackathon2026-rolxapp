use std::time::Duration;

use dioxus::prelude::*;
use dioxus_sdk_time::use_interval;

use crate::day_model::{self, LoadError};
use crate::platform::PlatformHandle;
use crate::state::AppState;
use crate::views::end_session;

const CHECK_PERIOD: Duration = Duration::from_secs(5 * 60);

/// Keeps the session alive while the app is open, so a long idle day view still works. A failed
/// attempt counts only if the server rejects the session: the next check simply tries again. On
/// native the first check runs right away, on wasm after one period; both are harmless, as the day
/// view checks by itself and extending twice only leaves the newer token in place.
#[component]
pub fn SessionKeeper() -> Element {
    let platform = use_context::<PlatformHandle>();
    let mut state = use_context::<AppState>();

    use_interval(CHECK_PERIOD, move |()| {
        let platform = platform.clone();
        async move {
            let Some(session) = state.session.peek().clone() else {
                return;
            };
            match day_model::fresh_session(&platform, session).await {
                Ok(session) => state.session.set(Some(session)),
                Err(LoadError::SessionRejected) => end_session(&platform, state),
                Err(LoadError::Failed(message)) => {
                    tracing::warn!(message, "could not extend the session");
                }
            }
        }
    });

    rsx! {}
}
