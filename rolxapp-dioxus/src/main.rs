use chrono::{Duration, Local, NaiveDate};
use dioxus::desktop::{Config, WindowBuilder};
use dioxus::prelude::*;
use rolx_core::client::RolxClient;
use rolx_core::config::AppConfig;
use rolx_core::login::{self, AuthFlow, UserAction};
use rolx_core::records::{self, RecordEntry, fmt_duration};
use rolx_core::session::{LoadError, Session};

const CSS: &str = include_str!("style.css");

fn main() {
    rolx_core::logging::init(0);
    let window = WindowBuilder::new().with_title("RolX");
    dioxus::LaunchBuilder::desktop()
        .with_cfg(Config::new().with_window(window))
        .launch(App);
}

enum Loaded {
    NeedsLogin,
    Day {
        user: String,
        entries: Vec<RecordEntry>,
    },
}

async fn load_day(day: NaiveDate) -> anyhow::Result<Loaded> {
    match Session::load() {
        Err(LoadError::NoSession) => return Ok(Loaded::NeedsLogin),
        Ok(session) if session.is_expired() => return Ok(Loaded::NeedsLogin),
        _ => {}
    }
    let config = AppConfig::load()?;
    let client = RolxClient::new(&config).await?;
    let user = format!(
        "{} {}",
        client.session.user_first_name, client.session.user_last_name
    );
    let entries = records::fetch_day(&client, day).await?;
    Ok(Loaded::Day { user, entries })
}

/// Logs in via the browser; `manual_url` receives the login URL if the browser could not be opened.
async fn log_in(mut manual_url: Signal<Option<String>>) -> anyhow::Result<()> {
    let config = AppConfig::load()?;
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
    let mut login_busy = use_signal(|| false);
    let mut login_error = use_signal(|| None::<String>);
    let mut login_url = use_signal(|| None::<String>);

    let loaded = use_resource(move || async move {
        refresh();
        load_day(day()).await
    });

    let on_login = move |_| {
        spawn(async move {
            login_busy.set(true);
            login_error.set(None);
            if let Err(e) = log_in(login_url).await {
                login_error.set(Some(e.to_string()));
            }
            login_busy.set(false);
            login_url.set(None);
            refresh += 1;
        });
    };

    let on_logout = move |_| {
        if let Err(e) = Session::delete() {
            login_error.set(Some(e.to_string()));
        }
        refresh += 1;
    };

    let content = match &*loaded.read() {
        None => rsx! { p { class: "empty", "Loading…" } },
        Some(Err(e)) => rsx! { p { class: "error", "{e}" } },
        Some(Ok(Loaded::NeedsLogin)) => rsx! {
            div { class: "card empty",
                p { "You are not logged in." }
                button { disabled: login_busy(), onclick: on_login,
                    if login_busy() { "Waiting for browser…" } else { "Log in with Google" }
                }
                if let Some(url) = login_url() {
                    p { "Opening the browser failed, open this link to log in: "
                        a { href: "{url}", "Log in" }
                    }
                }
            }
        },
        Some(Ok(Loaded::Day { user, entries })) => {
            let rows: Vec<_> = entries
                .iter()
                .map(|e| {
                    (
                        e.full_activity_number.clone(),
                        fmt_duration(e.duration),
                        e.comment.clone().unwrap_or_default(),
                    )
                })
                .collect();
            let total = fmt_duration(entries.iter().map(|e| e.duration).sum::<Duration>());
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
                div { class: "card",
                    if rows.is_empty() {
                        p { class: "empty", "No entries for this day." }
                    } else {
                        table {
                            thead { tr { th { "Activity" } th { "Time" } th { "Comment" } } }
                            tbody {
                                for (activity, time, comment) in rows {
                                    tr {
                                        td { "{activity}" }
                                        td { class: "num", "{time}" }
                                        td { "{comment}" }
                                    }
                                }
                                tr { class: "total", td { "Total" } td { class: "num", "{total}" } td {} }
                            }
                        }
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
            if let Some(e) = login_error() {
                p { class: "error", "{e}" }
            }
            {content}
        }
    }
}
