#[cfg(target_os = "android")]
mod android;

use anyhow::bail;
use chrono::{Duration, Local, NaiveDate};
use dioxus::prelude::*;
use rolx_core::client::RolxClient;
use rolx_core::config::AppConfig;
use rolx_core::login::{self, AuthFlow, UserAction};
use rolx_core::records::{self, RecordEntry, fmt_duration};
use rolx_core::session::{LoadError, Session};

const MAIN_CSS: Asset = asset!("/assets/main.css");
/// Opens the app from the browser, linked on the page shown after the Google login. The scheme is registered in
/// `android/AndroidManifest.xml`.
const RETURN_URL: &str = "ch.cudos.rolxmobile://login";
/// Config embedded at build time, see `build.rs`.
const CONFIG_TOML: &str = include_str!(concat!(env!("OUT_DIR"), "/config.toml"));

fn main() {
    dioxus::launch(App);
}

/// Points rolx-core to the app's files dir, as Android has no config and cache dirs. Elsewhere the defaults of the
/// CLI and desktop app are kept, so they share the session.
fn init_storage() {
    #[cfg(target_os = "android")]
    match android::files_dir() {
        Ok(dir) => {
            dioxus::logger::tracing::info!(dir = %dir.display(), "storing the session in");
            _ = rolx_core::paths::set_base_dir(dir);
        }
        Err(e) => dioxus::logger::tracing::error!(error = %e, "cannot determine the files dir"),
    }
}

fn config() -> anyhow::Result<AppConfig> {
    if CONFIG_TOML.trim().is_empty() {
        bail!("no RolX config embedded in the app, see docs/dev-setup.md");
    }
    Ok(AppConfig::from_toml(CONFIG_TOML)?)
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
    let client = RolxClient::new(&config()?).await?;
    let user = format!(
        "{} {}",
        client.session.user_first_name, client.session.user_last_name
    );
    let entries = records::fetch_day(&client, day).await?;
    Ok(Loaded::Day { user, entries })
}

/// What the user has to do to complete a running login.
#[derive(Clone)]
enum LoginPrompt {
    OpenUrl(String),
    EnterCode { url: String, code: String },
}

async fn log_in(flow: AuthFlow, mut prompt: Signal<Option<LoginPrompt>>) -> anyhow::Result<()> {
    login::run(&config()?, flow, |action| {
        prompt.set(Some(match action {
            UserAction::OpenUrl { url } => LoginPrompt::OpenUrl(url.into()),
            UserAction::EnterCode { url, code } => LoginPrompt::EnterCode {
                url: url.into(),
                code: code.into(),
            },
        }));
    })
    .await?;
    Ok(())
}

/// Renders `url` as a QR code SVG, so the login can be done on another device by scanning it.
fn qr_svg(url: &str) -> Option<String> {
    use qrcode::render::svg;
    let qr = qrcode::QrCode::new(url).ok()?;
    Some(qr.render::<svg::Color>().min_dimensions(200, 200).build())
}

#[component]
fn App() -> Element {
    use_hook(init_storage);

    let mut day = use_signal(|| Local::now().date_naive());
    let mut refresh = use_signal(|| 0u32);
    let mut login_busy = use_signal(|| false);
    let mut login_error = use_signal(|| None::<String>);
    let mut login_prompt = use_signal(|| None::<LoginPrompt>);

    let loaded = use_resource(move || async move {
        refresh();
        load_day(day()).await
    });

    let start_login = move |flow: AuthFlow| {
        spawn(async move {
            login_busy.set(true);
            login_error.set(None);
            if let Err(e) = log_in(flow, login_prompt).await {
                login_error.set(Some(e.to_string()));
            }
            login_busy.set(false);
            login_prompt.set(None);
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
        Some(Err(e)) => rsx! {
            div { class: "card",
                p { class: "error", "{e}" }
                button { onclick: move |_| refresh += 1, "Retry" }
            }
        },
        Some(Ok(Loaded::NeedsLogin)) => rsx! {
            div { class: "card login",
                p { "You are not logged in." }
                button { disabled: login_busy(), onclick: move |_| start_login(AuthFlow::NativeApp { return_url: Some(RETURN_URL.into()) }),
                    if login_busy() { "Waiting for login…" } else { "Log in with Google" }
                }
                button { class: "secondary", disabled: login_busy(), onclick: move |_| start_login(AuthFlow::Device),
                    "Log in with a code"
                }
                match login_prompt() {
                    Some(LoginPrompt::OpenUrl(url)) => rsx! {
                        p { "Opening the browser failed, open this link to log in: "
                            a { href: "{url}", "Log in" }
                        }
                    },
                    Some(LoginPrompt::EnterCode { url, code }) => rsx! {
                        p { "Open " a { href: "{url}", "{url}" } " or scan the QR code, and enter this code:" }
                        if let Some(svg) = qr_svg(&url) {
                            div { class: "qr", dangerous_inner_html: svg }
                        }
                        p { class: "code", "{code}" }
                    },
                    None => rsx! {},
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
                }
                div { class: "actions",
                    button { class: "secondary", onclick: move |_| day.set(Local::now().date_naive()), "Today" }
                    button { onclick: move |_| refresh += 1, "Refresh" }
                }
                div { class: "card",
                    if rows.is_empty() {
                        p { class: "empty", "No entries for this day." }
                    } else {
                        ul { class: "entries",
                            for (activity, time, comment) in rows {
                                li {
                                    div { class: "entry-head",
                                        span { class: "activity", "{activity}" }
                                        span { class: "num", "{time}" }
                                    }
                                    if !comment.is_empty() {
                                        div { class: "comment", "{comment}" }
                                    }
                                }
                            }
                            li { class: "total entry-head",
                                span { "Total" }
                                span { class: "num", "{total}" }
                            }
                        }
                    }
                }
            }
        }
    };

    let logged_in = matches!(&*loaded.read(), Some(Ok(Loaded::Day { .. })));

    rsx! {
        document::Meta { name: "viewport", content: "width=device-width, initial-scale=1" }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
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
