use chrono::{Duration, Local, NaiveDate};
use dioxus::core::Task;
use dioxus::desktop::{Config, LogicalPosition, LogicalSize, WindowBuilder};
use dioxus::prelude::*;
use rolx_core::activities::{self, Activity};
use rolx_core::config::AppConfig;
use rolx_core::dayedit::load_day as load_day_entries;
use rolx_core::dayplan::DayEntry;
use rolx_core::login::{self, AuthFlow, UserAction};
use rolx_core::stamp::StampState;

mod activity_picker;
mod day_view;
mod settings;
mod shared;
mod stamp_panel;
mod week_panel;
use day_view::DayView;
use stamp_panel::StampPanel;
use week_panel::WeekPanel;

const CSS: &str = include_str!("style.css");

// Window defaults (logical pixels). The position is measured from the top-left corner of the primary
// screen, so the window always opens on it.
const WINDOW_SIZE: (f64, f64) = (900.0, 760.0);
const WINDOW_MIN_SIZE: (f64, f64) = (480.0, 400.0);
const WINDOW_POSITION: (f64, f64) = (60.0, 40.0);

fn main() {
    rolx_core::logging::init(0);
    let window = WindowBuilder::new()
        .with_title("RolX")
        .with_inner_size(LogicalSize::new(WINDOW_SIZE.0, WINDOW_SIZE.1))
        .with_min_inner_size(LogicalSize::new(WINDOW_MIN_SIZE.0, WINDOW_MIN_SIZE.1))
        .with_position(LogicalPosition::new(WINDOW_POSITION.0, WINDOW_POSITION.1));
    dioxus::LaunchBuilder::desktop()
        .with_cfg(Config::new().with_window(window))
        .launch(App);
}

enum Loaded {
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

/// Logs in via the browser; `manual_url` receives the login URL if the browser could not be opened.
async fn log_in(mut manual_url: Signal<Option<String>>) -> anyhow::Result<()> {
    let config = AppConfig::load_with_env()?;
    login::run(
        &config,
        AuthFlow::NativeApp { return_url: None },
        |action| {
            if let UserAction::OpenUrl { url } = action {
                manual_url.set(Some(url.to_string()));
            }
        },
    )
    .await?;
    Ok(())
}

#[component]
fn App() -> Element {
    let mut day = use_signal(|| Local::now().date_naive());
    let mut refresh = use_signal(|| 0u32);
    // The running login, if any; cancelling it drops the flow, which closes the redirect listener.
    let mut login_task = use_signal(|| None::<Task>);
    let mut login_url = use_signal(|| None::<String>);
    let mut notice = use_signal(|| None::<String>);

    let stamp_state = use_signal(|| StampState::load().unwrap_or_default());
    let mut activities = use_signal(|| activities::load_cached().unwrap_or_default());
    let selected = use_signal(|| None::<i32>);

    let loaded = use_resource(move || async move {
        refresh();
        load_day(day()).await
    });

    let on_favourite = move |(id, favourite): (i32, bool)| {
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
    };

    let on_reload = move |_| {
        spawn(async move {
            match shared::reload_activities().await {
                Ok(list) => {
                    notice.set(None);
                    activities.set(list);
                }
                Err(e) => notice.set(Some(format!("Could not reload the activities: {e}"))),
            }
        });
    };

    // Keep the list complete without the user having to ask: once at start, if logged in.
    use_future(move || async move {
        if shared::has_session()
            && let Ok(list) = shared::reload_activities().await
        {
            activities.set(list);
        }
    });

    let on_login = move |_| {
        notice.set(None);
        let task = spawn(async move {
            if let Err(e) = log_in(login_url).await {
                notice.set(Some(e.to_string()));
            }
            login_task.set(None);
            login_url.set(None);
            refresh += 1;
        });
        login_task.set(Some(task));
    };

    let cancel_login = move |_| {
        if let Some(task) = login_task.take() {
            task.cancel();
        }
        login_url.set(None);
    };

    let on_logout = move |_| {
        if let Err(e) = rolx_core::session::Session::delete() {
            notice.set(Some(e.to_string()));
        }
        refresh += 1;
    };

    let login_busy = login_task().is_some();
    let content = match &*loaded.read() {
        None => rsx! { p { class: "empty", "Loading…" } },
        Some(Err(e)) => rsx! { p { class: "error", "{e}" } },
        Some(Ok(Loaded::NeedsLogin)) => rsx! {
            div { class: "card empty",
                p { "You are not logged in." }
                button { disabled: login_busy, onclick: on_login,
                    if login_busy { "Waiting for browser…" } else { "Log in with Google" }
                }
                if login_busy {
                    button { class: "secondary", onclick: cancel_login, "Cancel" }
                }
                if let Some(url) = login_url() {
                    p { "Opening the browser failed, open this link to log in: "
                        a { href: "{url}", "Log in" }
                    }
                }
            }
        },
        Some(Ok(Loaded::Day { user, entries })) => {
            let date_label = day().format("%a %d.%m.%Y").to_string();
            rsx! {
                p { class: "user", "Signed in as {user}" }
                div { class: "card day-nav",
                    button { class: "secondary", onclick: move |_| day.set(day() - Duration::days(1)), "◀" }
                    span { class: "date", "{date_label}" }
                    button { class: "secondary", onclick: move |_| day.set(day() + Duration::days(1)), "▶" }
                    button { class: "secondary", onclick: move |_| day.set(Local::now().date_naive()), "Today" }
                    button { onclick: move |_| refresh += 1, "Refresh" }
                }
                WeekPanel { day, refresh, state: stamp_state }
                for d in [day()] {
                    DayView {
                        key: "{d}",
                        day: d,
                        entries: entries.clone(),
                        state: stamp_state,
                        activities,
                        on_favourite,
                        on_reload,
                        on_changed: move |_| refresh += 1,
                    }
                }
            }
        }
    };

    let logged_in = matches!(&*loaded.read(), Some(Ok(Loaded::Day { .. })));

    rsx! {
        style { "{CSS}" }
        main {
            header {
                h1 { "RolX" }
                if logged_in {
                    button { class: "secondary", onclick: on_logout, "Log out" }
                }
            }
            if let Some(e) = notice() {
                p { class: "error", "{e}" }
            }
            StampPanel {
                state: stamp_state,
                activities,
                selected,
                on_uploaded: move |_| refresh += 1,
                on_favourite,
                on_reload,
            }
            {content}
        }
    }
}
