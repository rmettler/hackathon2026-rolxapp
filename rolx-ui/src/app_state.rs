//! The state an app sets up once at the top: the shown day, stamping, the activity list and the
//! messages, with the handlers the components share.

use chrono::{Local, NaiveDate};
use dioxus::prelude::*;
use rolx_core::activities::{self, Activity};
use rolx_core::dayedit::load_day as load_day_entries;
use rolx_core::dayplan::DayEntry;
use rolx_core::stamp::StampState;

use crate::shared;
use crate::stamper::{Stamper, spawn_in_app};

/// What the main view shows.
pub enum Loaded {
    NeedsLogin,
    Day {
        user: String,
        entries: Vec<DayEntry>,
    },
}

async fn load_day(day: NaiveDate) -> anyhow::Result<Loaded> {
    if !shared::has_session() {
        return Ok(Loaded::NeedsLogin);
    }
    let client = shared::client().await?;
    let user = format!(
        "{} {}",
        client.session.user_first_name, client.session.user_last_name
    );
    let entries = load_day_entries(&client, day).await?;
    Ok(Loaded::Day { user, entries })
}

#[derive(Clone, Copy)]
pub struct AppState {
    pub day: Signal<NaiveDate>,
    /// Bump it to reload the day and the week.
    pub refresh: Signal<u32>,
    pub stamp_state: Signal<StampState>,
    pub activities: Signal<Vec<Activity>>,
    /// The activity chosen in the stamp panel.
    pub selected: Signal<Option<i32>>,
    /// An error to show at the top.
    pub notice: Signal<Option<String>>,
    /// A confirmation to show at the top, e.g. what was undone.
    pub info: Signal<Option<String>>,
    /// Also provided as context, for the components that stamp.
    pub stamper: Stamper,
    /// The shown day, or that a login is needed. Reloads when `day` or `refresh` change.
    pub loaded: Resource<anyhow::Result<Loaded>>,
    /// Pins or unpins a favourite activity (`ActivityPicker::on_favourite`).
    pub on_favourite: Callback<(i32, bool)>,
    /// Downloads the activity list again (`ActivityPicker::on_reload`).
    pub on_reload: Callback<()>,
}

/// Sets up the app state; call it once, in the root component. Also reloads the activity list
/// once at start if logged in.
pub fn use_app_state() -> AppState {
    let day = use_signal(|| Local::now().date_naive());
    let refresh = use_signal(|| 0u32);
    let mut notice = use_signal(|| None::<String>);
    let info = use_signal(|| None::<String>);

    let stamp_state = use_signal(|| StampState::load().unwrap_or_default());
    let mut activities = use_signal(|| activities::load_cached().unwrap_or_default());
    let selected = use_signal(|| None::<i32>);

    let message = use_signal(|| None::<String>);
    let syncing = use_signal(|| false);
    let stamper = use_context_provider(|| Stamper {
        state: stamp_state,
        activities,
        refresh,
        message,
        syncing,
    });

    let loaded = use_resource(move || async move {
        refresh();
        load_day(day()).await
    });

    let on_favourite = use_callback(move |(id, favourite): (i32, bool)| {
        let found = activities.read().iter().find(|a| a.id == id).cloned();
        let Some(activity): Option<Activity> = found else {
            return;
        };
        spawn(async move {
            match shared::set_favourite(activity, favourite).await {
                Ok(()) => {
                    notice.set(None);
                    activities.set(activities::load_cached().unwrap_or_default());
                }
                Err(e) => notice.set(Some(format!("Could not change the favourite: {e}"))),
            }
        });
    });

    let on_reload = use_callback(move |_| {
        // Not tied to the caller: the palette closes itself right after calling this.
        spawn_in_app(async move {
            match shared::reload_activities().await {
                Ok(list) => {
                    notice.set(None);
                    activities.set(list);
                }
                Err(e) => notice.set(Some(format!("Could not reload the activities: {e}"))),
            }
        });
    });

    // Keep the list complete without the user having to ask: once at start, if logged in.
    use_future(move || async move {
        if shared::has_session()
            && let Ok(list) = shared::reload_activities().await
        {
            activities.set(list);
        }
    });

    AppState {
        day,
        refresh,
        stamp_state,
        activities,
        selected,
        notice,
        info,
        stamper,
        loaded,
        on_favourite,
        on_reload,
    }
}
