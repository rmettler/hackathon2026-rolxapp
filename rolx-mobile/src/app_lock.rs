//! The app lock: when turned on in the settings, the app shows only a lock screen at start and after it was in the
//! background for [`LOCK_AFTER_SECS`], until unlocked with a fingerprint, the face or the phone's screen lock. It
//! guards the window only; the session stays readable, so the widget and the background upload keep working.

use chrono::{DateTime, Duration, Utc};
use dioxus::prelude::*;
use rolx_ui::lucide::{Glyph, Icon};
use rolx_ui::stamper::spawn_in_app;

use crate::plugin;
use crate::settings::Settings;

/// How long the app may be in the background before it locks again.
const LOCK_AFTER_SECS: i64 = 60;

#[derive(Clone, Copy, PartialEq)]
pub struct AppLock {
    /// The setting.
    pub enabled: Signal<bool>,
    pub locked: Signal<bool>,
    /// The system's prompt is open.
    pub prompting: Signal<bool>,
    pub error: Signal<Option<String>>,
    /// Counts refused attempts to turn the lock on, so the settings redraw the checkbox the user ticked.
    pub refused: Signal<u32>,
    /// Wall-clock time, as the phone's monotonic clock stands still in deep sleep.
    hidden_since: Signal<Option<DateTime<Utc>>>,
}

/// The app lock's state; locked at start when it is on. Call it once, in the root component.
pub fn use_app_lock() -> AppLock {
    let on = use_hook(|| Settings::load().app_lock);
    AppLock {
        enabled: use_signal(|| on),
        locked: use_signal(|| on),
        prompting: use_signal(|| false),
        error: use_signal(|| None),
        refused: use_signal(|| 0),
        hidden_since: use_signal(|| None),
    }
}

impl AppLock {
    /// The app went to the background.
    pub fn hidden(mut self) {
        if *self.enabled.peek() && !*self.locked.peek() {
            self.hidden_since.set(Some(Utc::now()));
        }
    }

    /// The app is back in the foreground: locks it if it was away long enough, and asks again if it is locked.
    pub fn visible(mut self) {
        let since = self.hidden_since.take();
        if !*self.enabled.peek() {
            return;
        }
        if since.is_some_and(|t| Utc::now() - t >= Duration::seconds(LOCK_AFTER_SECS)) {
            // The lock screen asks when it shows.
            self.locked.set(true);
        } else if *self.locked.peek() {
            self.unlock();
        }
    }

    /// Shows the system's prompt (once at a time) and unlocks the app if the user passes it.
    pub fn unlock(mut self) {
        if *self.prompting.peek() {
            return;
        }
        self.prompting.set(true);
        self.error.set(None);
        // In the app's scope: the lock screen that asked goes away when unlocked.
        spawn_in_app(async move {
            let result =
                plugin::unlock("Unlock RolX", "Use your fingerprint, face or screen lock").await;
            self.prompting.set(false);
            match result {
                Ok(true) => self.locked.set(false),
                Ok(false) => {}
                Err(e) => self
                    .error
                    .set(Some(format!("Cannot show the unlock prompt: {e}"))),
            }
        });
    }

    /// Turns the lock on, once the user has unlocked with the prompt (so it cannot lock them out), or off.
    pub fn set_enabled(mut self, on: bool) {
        if !on {
            self.enabled.set(false);
            Settings::update(|s| s.app_lock = false);
            return;
        }
        if *self.prompting.peek() {
            return;
        }
        self.prompting.set(true);
        self.error.set(None);
        spawn_in_app(async move {
            let result = plugin::unlock("Turn on the app lock", "Confirm it is you").await;
            self.prompting.set(false);
            match result {
                Ok(true) => {
                    self.enabled.set(true);
                    Settings::update(|s| s.app_lock = true);
                }
                Ok(false) => self.refused += 1,
                Err(e) => {
                    self.refused += 1;
                    self.error
                        .set(Some(format!("Cannot show the unlock prompt: {e}")));
                }
            }
        });
    }
}

/// Covers the app while it is locked, and asks to unlock as soon as it shows.
#[component]
pub fn LockScreen(lock: AppLock) -> Element {
    use_hook(move || lock.unlock());
    let prompting = (lock.prompting)();
    rsx! {
        div {
            class: "lock-screen",
            role: "dialog",
            "aria-modal": "true",
            "aria-labelledby": "lock-title",
            Icon { glyph: Glyph::Lock, size: 48 }
            h1 { id: "lock-title", "RolX is locked" }
            p { "Unlock with your fingerprint, face or screen lock." }
            button { disabled: prompting, onclick: move |_| lock.unlock(), "Unlock" }
            if let Some(e) = (lock.error)() {
                p { class: "error", role: "alert", "{e}" }
            }
        }
    }
}
