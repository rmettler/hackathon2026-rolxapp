//! Stamping actions shared by everything that can start or stop work: the stamp panel, the command
//! palette, the tray menu and the global hotkey.

use chrono::{Local, NaiveDateTime};
use dioxus::prelude::*;
use rolx_core::activities::{self, Activity};
use rolx_core::stamp::{ActivityRef, StampState};
use rolx_core::sync::{self, SyncReport};

use crate::shared;

pub type Action = Box<dyn FnOnce(&mut StampState, NaiveDateTime) -> anyhow::Result<()>>;

/// The user chose to keep a long-running session (so the forgotten-stamp banner stays away).
pub static STALE_DISMISSED: GlobalSignal<bool> = Signal::global(|| false);

/// Applies an action to the state on disk (not to the copy in memory), so changes made by the
/// CLI in the meantime are never overwritten.
pub fn mutate(action: Action) -> anyhow::Result<StampState> {
    let (state, ()) = StampState::update(|state| action(state, Local::now().naive_local()))?;
    Ok(state)
}

/// Uploads the queue. `None` when there is no valid login, since stamping works without one.
async fn sync_all() -> anyhow::Result<Option<SyncReport>> {
    if !shared::has_session() {
        return Ok(None);
    }
    let client = shared::client().await?;
    Ok(Some(sync::run_default(&client).await?))
}

/// Runs the future in the app's scope. It then outlives the component that started it (banners and
/// the palette disappear right after being used) and uses the app's signals from their own scope;
/// `spawn_forever` would run in the root scope and trigger Dioxus' "signal used outside its owner" warning.
pub fn spawn_in_app(future: impl Future<Output = ()> + 'static) {
    dioxus::core::Runtime::current().spawn(dioxus::core::ScopeId::APP, future);
}

#[derive(Clone, Copy)]
pub struct Stamper {
    pub state: Signal<StampState>,
    pub activities: Signal<Vec<Activity>>,
    pub refresh: Signal<u32>,
    pub message: Signal<Option<String>>,
    pub syncing: Signal<bool>,
}

impl Stamper {
    pub fn activity_ref(&self, id: i32) -> Option<ActivityRef> {
        self.activities
            .read()
            .iter()
            .find(|a| a.id == id)
            .map(Activity::to_ref)
    }

    /// A session that looks forgotten and has not been waved through.
    pub fn blocked(&self) -> bool {
        self.state
            .read()
            .stale(Local::now().naive_local())
            .is_some()
            && !STALE_DISMISSED()
    }

    /// Runs the action unless a forgotten session has to be dealt with first.
    pub fn run(mut self, action: Action) {
        if self.blocked() {
            self.message
                .set(Some("Deal with the forgotten session first".into()));
            return;
        }
        self.run_always(action);
    }

    pub fn run_always(mut self, action: Action) {
        match mutate(action) {
            Ok(new_state) => {
                self.message.set(None);
                let uploadable = new_state.pending.iter().any(|e| e.activity.is_some());
                self.state.set(new_state);
                if uploadable {
                    self.sync(true);
                }
            }
            Err(e) => self.message.set(Some(e.to_string())),
        }
    }

    /// `quiet` is for the automatic upload after stamping: no message unless something went wrong.
    pub fn sync(mut self, quiet: bool) {
        // Not tied to the calling component: banners and the palette disappear when used.
        spawn_in_app(async move {
            self.syncing.set(true);
            match sync_all().await {
                Ok(Some(report)) => {
                    if let Some((day, reason)) = report.failed.first() {
                        self.message
                            .set(Some(format!("{day} not uploaded: {reason}")));
                    } else if !quiet {
                        self.message
                            .set(Some(format!("Uploaded {} entries", report.uploaded)));
                    } else {
                        self.message.set(None);
                    }
                }
                Ok(None) if !quiet => self
                    .message
                    .set(Some("Log in to upload your entries".into())),
                Ok(None) => {}
                Err(e) => self
                    .message
                    .set(Some(format!("Upload failed, will retry later: {e}"))),
            }
            self.state.set(StampState::load().unwrap_or_default());
            self.activities
                .set(activities::load_cached().unwrap_or_default());
            self.syncing.set(false);
            self.refresh += 1;
        });
    }

    /// Starts working on the activity, or switches to it when already stamped in. `report` replaces
    /// the work report of the session that ends.
    pub fn start(self, activity_id: i32, report: Option<String>) {
        let Some(activity) = self.activity_ref(activity_id) else {
            return;
        };
        self.run(Box::new(move |s, now| {
            if s.open.is_some() {
                if let Some(text) = report {
                    s.set_open_comment(Some(text))?;
                }
                s.switch(now, Some(activity), None)?;
            } else {
                s.stamp_in(now, Some(activity), report);
            }
            Ok(())
        }));
    }

    /// Stamps out when stamped in; otherwise stamps in with the activity used last.
    pub fn toggle(self) {
        let activities = self.activities.read().clone();
        self.run(Box::new(move |s, now| {
            s.toggle(now, |id| {
                activities.iter().find(|a| a.id == id).map(Activity::to_ref)
            })?;
            Ok(())
        }));
    }
}
