//! The app's side of the Kotlin parts (`bridge.rs`): keeps the home-screen widget and the background upload in step
//! with the stamps, and asks for the app lock's prompt. Off Android the calls do nothing, so the app still builds and
//! runs on the desktop (without an app lock).

use std::cell::Cell;
use std::rc::Rc;

use dioxus::logger::tracing::warn;
use dioxus::prelude::*;
use rolx_core::stamp::StampState;

/// Sets up the hourly background upload once, and on every change of the stamps redraws the widgets and, when there
/// is more to upload than before, has the upload run once a network is there (the app uploads right away too, but may
/// be closed before it gets a network). Call it once, in the root component, with `AppState::stamp_state`.
pub fn use_plugin_sync(state: Signal<StampState>) {
    use_hook(|| {
        log_error(
            "setting up the background upload",
            platform::start_uploads(),
        )
    });
    // Not a signal: the effect must not depend on it.
    let uploadable_before = use_hook(|| Rc::new(Cell::new(0usize)));
    use_effect(move || {
        let uploadable = state.read().uploadable_count();
        log_error("refreshing the widgets", platform::refresh_widgets());
        if uploadable > uploadable_before.replace(uploadable) {
            log_error("scheduling the upload", platform::schedule_upload());
        }
    });
}

/// Whether the phone has a fingerprint, face or screen lock to unlock the app with.
pub fn lock_available() -> bool {
    platform::lock_available()
}

/// Shows the system's unlock prompt and waits for the answer: whether the user unlocked.
pub async fn unlock(title: &str, subtitle: &str) -> anyhow::Result<bool> {
    platform::unlock(title, subtitle).await
}

fn log_error(what: &str, result: anyhow::Result<()>) {
    if let Err(e) = result {
        warn!(error = %e, "{what}");
    }
}

#[cfg(target_os = "android")]
mod platform {
    use std::time::Duration;

    use crate::bridge;

    /// How often the prompt's answer is checked; it comes from the UI thread and is not reported to Rust.
    const UNLOCK_POLL: Duration = Duration::from_millis(100);

    pub fn start_uploads() -> anyhow::Result<()> {
        bridge::start_uploads()
    }

    pub fn refresh_widgets() -> anyhow::Result<()> {
        bridge::refresh_widgets()
    }

    pub fn schedule_upload() -> anyhow::Result<()> {
        bridge::schedule_upload()
    }

    pub fn lock_available() -> bool {
        bridge::lock_available().unwrap_or(false)
    }

    pub async fn unlock(title: &str, subtitle: &str) -> anyhow::Result<bool> {
        bridge::request_unlock(title, subtitle)?;
        loop {
            tokio::time::sleep(UNLOCK_POLL).await;
            match bridge::unlock_status()? {
                bridge::UNLOCK_PENDING => {}
                status => return Ok(status == bridge::UNLOCKED),
            }
        }
    }
}

#[cfg(not(target_os = "android"))]
mod platform {
    pub fn start_uploads() -> anyhow::Result<()> {
        Ok(())
    }

    pub fn refresh_widgets() -> anyhow::Result<()> {
        Ok(())
    }

    pub fn schedule_upload() -> anyhow::Result<()> {
        Ok(())
    }

    pub fn lock_available() -> bool {
        false
    }

    pub async fn unlock(_title: &str, _subtitle: &str) -> anyhow::Result<bool> {
        Ok(true)
    }
}
