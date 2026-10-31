#[cfg(target_os = "android")]
mod android;
mod app_lock;
#[cfg(target_os = "android")]
mod bridge;
mod links;
mod native;
mod plugin;
#[cfg(any(target_os = "android", test))]
mod widget;

use anyhow::bail;
use chrono::{Duration, Local};
use dioxus::core::Task;
use dioxus::prelude::*;
use rolx_core::config::AppConfig;
use rolx_core::idle::Away;
use rolx_core::login::{self, AuthFlow, UserAction};
use rolx_core::session::Session;
use rolx_core::stamp::StampState;
use rolx_core::week::{fetch_week, summarize};
use rolx_ui::Compact;
use rolx_ui::app_state::{AppState, Loaded, use_app_state};
use rolx_ui::day_view::DayView;
use rolx_ui::lucide::{Glyph, Icon};
use rolx_ui::stamp_panel::StampPanel;
use rolx_ui::stamper::Stamper;
use rolx_ui::undo::{UndoLine, use_undo};
use rolx_ui::week_panel::WeekPanel;

mod settings;
use app_lock::{AppLock, LockScreen, use_app_lock};
use settings::Settings;

/// Mobile layout on top of `rolx_ui::CSS`.
const CSS: &str = include_str!("mobile.css");
/// The brand font's `@font-face` rules, empty when its files were not there at build time (see `build.rs`).
const FONTS_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/fonts.css"));
/// Opens the app from the browser, linked on the page shown after the Google login. The scheme is registered in
/// `android/AndroidManifest.xml`.
const RETURN_URL: &str = "ch.cudos.rolxmobile://login";
/// Config embedded at build time, see `build.rs`. Also used by `bridge.rs` when Kotlin calls in without the app.
const CONFIG_TOML: &str = include_str!(concat!(env!("OUT_DIR"), "/config.toml"));

fn main() {
    let _ = rolx_ui::set_config_source(config);
    dioxus::launch(App);
}

/// Points rolx-core to the app's files dir, as Android has no config, cache and data dirs. Elsewhere the defaults of the
/// CLI and desktop app are kept, so they share the session.
fn init_storage() {
    #[cfg(target_os = "android")]
    match android::files_dir() {
        Ok(dir) => {
            dioxus::logger::tracing::info!(dir = %dir.display(), "storing the session and data in");
            _ = rolx_core::paths::set_base_dir(dir);
        }
        Err(e) => dioxus::logger::tracing::error!(error = %e, "cannot determine the files dir"),
    }
}

pub(crate) fn config() -> anyhow::Result<AppConfig> {
    if CONFIG_TOML.trim().is_empty() {
        bail!("no RolX config embedded in the app, see docs/dev-setup.md");
    }
    Ok(AppConfig::from_toml(CONFIG_TOML)?)
}

/// The logged-in user's name, if the session is valid.
fn session_user() -> Option<String> {
    Session::load()
        .ok()
        .filter(|s| !s.is_expired())
        .map(|s| format!("{} {}", s.user_first_name, s.user_last_name))
}

/// What the user has to do to complete a running login.
#[derive(Clone, PartialEq)]
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

/// A running login and what it needs from the user. Kept in the app, so switching tabs doesn't lose it.
#[derive(Clone, Copy)]
struct LoginState {
    /// Cancelling it drops the flow, which closes the redirect listener or stops polling.
    task: Signal<Option<Task>>,
    error: Signal<Option<String>>,
    prompt: Signal<Option<LoginPrompt>>,
    start: Callback<AuthFlow>,
    cancel: Callback<()>,
}

/// The card shown instead of the day and the week without a session; stamping works without one.
fn login_card(login: LoginState) -> Element {
    let busy = login.task.read().is_some();
    rsx! {
        div { class: "card login",
            p { "You are not logged in." }
            button { disabled: busy, onclick: move |_| login.start.call(AuthFlow::NativeApp { return_url: Some(RETURN_URL.into()) }),
                if busy { "Waiting for login…" } else { "Log in with Google" }
            }
            button { class: "secondary", disabled: busy, onclick: move |_| login.start.call(AuthFlow::Device),
                "Log in with a code"
            }
            if busy {
                button { class: "secondary", onclick: move |_| login.cancel.call(()), "Cancel" }
            }
            match (login.prompt)() {
                Some(LoginPrompt::OpenUrl(url)) => rsx! {
                    p { "Opening the browser failed, open this link to log in: "
                        a { href: "{url}", "Log in" }
                    }
                },
                Some(LoginPrompt::EnterCode { url, code }) => rsx! {
                    p { "Open " a { href: "{url}", "{url}" } " or scan the QR code, and enter this code:" }
                    if let Some(svg) = qr_svg(&url) {
                        div {
                            class: "qr",
                            role: "img",
                            "aria-label": "QR code of the login link",
                            dangerous_inner_html: svg,
                        }
                    }
                    p { class: "code", "{code}" }
                },
                None => rsx! {},
            }
            if let Some(e) = (login.error)() {
                p { class: "error", role: "alert", "{e}" }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Stamp,
    Day,
    Week,
}

/// Page events the app reacts to: going to the background and coming back, the network, and swipes (to change the
/// day).
const PAGE_EVENTS: &str = r#"
// The UI is English; screen readers pick their voice from this.
document.documentElement.lang = 'en';
const send = (m) => dioxus.send(m);
document.addEventListener('visibilitychange', () => {
  send(document.visibilityState === 'visible' ? 'visible' : 'hidden');
});
window.addEventListener('online', () => send('online'));
window.addEventListener('offline', () => send('offline'));
send(navigator.onLine ? 'started' : 'offline');

let swipe = null;
document.addEventListener('touchstart', (e) => {
  const t = e.target;
  // Anywhere but on what scrolls or takes input sideways; the app only acts on it on the Day tab.
  const free = e.touches.length === 1 && t.closest
    && !t.closest('input, textarea, select, .popover, .editing, .conflict, .chart, .tabbar, .sheet');
  swipe = free ? { x: e.touches[0].clientX, y: e.touches[0].clientY, at: Date.now() } : null;
}, { passive: true });
document.addEventListener('touchend', (e) => {
  if (!swipe) return;
  const t = e.changedTouches[0];
  const dx = t.clientX - swipe.x, dy = t.clientY - swipe.y;
  if (Date.now() - swipe.at < 800 && Math.abs(dx) > 60 && Math.abs(dx) > 2 * Math.abs(dy)) {
    send(dx < 0 ? 'next-day' : 'previous-day');
  }
  swipe = null;
}, { passive: true });
"#;

/// Whether there are finished entries with an activity, which an upload would send.
fn has_uploads(state: Signal<StampState>) -> bool {
    state.read().pending.iter().any(|e| e.activity.is_some())
}

#[component]
fn App() -> Element {
    use_hook(init_storage);
    use_context_provider(|| Compact(true));
    let app = use_app_state();
    let AppState {
        mut day,
        mut refresh,
        mut stamp_state,
        activities,
        selected,
        mut notice,
        info,
        stamper,
        loaded,
        on_favourite,
        on_reload,
    } = app;
    let undo = use_undo(app);
    native::use_native_stamp_sync(stamp_state);
    plugin::use_plugin_sync(stamp_state);
    let lock = use_app_lock();
    let lock_available = use_hook(plugin::lock_available);
    let mut tab = use_signal(|| Tab::Stamp);
    let mut show_settings = use_signal(|| false);
    // A stamp link (app shortcut, QR code, Tasker) shows what it did, or why not, on the stamp panel.
    links::use_stamp_links(
        stamper,
        use_callback(move |_| {
            tab.set(Tab::Stamp);
            show_settings.set(false);
        }),
    );
    let mut confirm_logout = use_signal(|| false);
    let mut online = use_signal(|| true);
    // Stamping has no away detection on the phone, so this stays empty.
    let away = use_signal(|| None::<Away>);
    let user = use_memo(move || {
        refresh();
        session_user()
    });

    let show_chart = use_signal(|| Settings::load().show_chart);
    use_effect(move || {
        let show = show_chart();
        if show != Settings::load().show_chart {
            Settings::update(|s| s.show_chart = show);
        }
    });

    let mut login_task = use_signal(|| None::<Task>);
    let mut login_error = use_signal(|| None::<String>);
    let mut login_prompt = use_signal(|| None::<LoginPrompt>);
    let start_login = use_callback(move |flow: AuthFlow| {
        login_error.set(None);
        let task = spawn(async move {
            match log_in(flow, login_prompt).await {
                Ok(()) => {
                    // Fetch what a logged-out phone could not: the activities, and upload what was stamped meanwhile.
                    on_reload.call(());
                    stamper.sync(true);
                }
                Err(e) => login_error.set(Some(e.to_string())),
            }
            login_task.set(None);
            login_prompt.set(None);
            refresh += 1;
        });
        login_task.set(Some(task));
    });
    let cancel_login = use_callback(move |_| {
        if let Some(task) = login_task.take() {
            task.cancel();
        }
        login_prompt.set(None);
    });
    let login = LoginState {
        task: login_task,
        error: login_error,
        prompt: login_prompt,
        start: start_login,
        cancel: cancel_login,
    };

    use_future(move || async move {
        let mut events = document::eval(PAGE_EVENTS);
        while let Ok(event) = events.recv::<String>().await {
            match event.as_str() {
                // Back from the background: the stamps may have changed (the widget, another app, a file edit), and
                // the day, the week and the server may have moved on.
                "visible" => {
                    lock.visible();
                    links::check_queue();
                    stamp_state.set(StampState::load().unwrap_or_default());
                    if online() && has_uploads(stamp_state) {
                        stamper.sync(true);
                    } else {
                        refresh += 1;
                    }
                }
                "started" | "online" => {
                    online.set(true);
                    if has_uploads(stamp_state) {
                        stamper.sync(true);
                    }
                }
                "hidden" => lock.hidden(),
                "offline" => online.set(false),
                "next-day" if tab() == Tab::Day => day.set(day() + Duration::days(1)),
                "previous-day" if tab() == Tab::Day => day.set(day() - Duration::days(1)),
                _ => {}
            }
        }
    });

    let share_day = move |_| {
        if let Some(Ok(Loaded::Day { entries, .. })) = &*loaded.read() {
            let text = native::day_share(day(), entries, &activities.read());
            if let Err(e) = native::share_text(&text.subject, &text.text) {
                notice.set(Some(format!("Could not share the day: {e}")));
            }
        }
    };
    // The week panel keeps its data to itself, so the week is fetched again for the text.
    let share_week = move |_| {
        spawn(async move {
            let result = async {
                let client = rolx_ui::shared::client().await?;
                let week = fetch_week(&client, day()).await?;
                let summary = summarize(&week, &stamp_state.read().pending);
                let text = native::week_share(&summary, &activities.read());
                native::share_text(&text.subject, &text.text)
            }
            .await;
            if let Err(e) = result {
                notice.set(Some(format!("Could not share the week: {e}")));
            }
        });
    };

    let on_logout = move |_| {
        if let Err(e) = Session::delete() {
            login_error.set(Some(e.to_string()));
        }
        confirm_logout.set(false);
        show_settings.set(false);
        refresh += 1;
    };

    if (lock.locked)() {
        return rsx! {
            style { "{FONTS_CSS}" }
            style { "{rolx_ui::CSS}" }
            style { "{CSS}" }
            LockScreen { lock }
        };
    }

    let logged_in = user().is_some();
    let date_label = day().format("%a %d.%m.%Y").to_string();
    let day_content = match &*loaded.read() {
        None => rsx! { p { class: "empty", "Loading…" } },
        Some(Err(e)) => rsx! {
            div { class: "card",
                p { class: "error", "{e}" }
                button { onclick: move |_| refresh += 1, "Retry" }
            }
        },
        Some(Ok(Loaded::NeedsLogin)) => login_card(login),
        Some(Ok(Loaded::Day { entries, .. })) => rsx! {
            for d in [day()] {
                DayView {
                    key: "{d}",
                    day: d,
                    entries: entries.clone(),
                    state: stamp_state,
                    activities,
                    on_favourite,
                    on_reload,
                    on_changed: undo.on_changed,
                }
            }
        },
    };

    let tab_class = move |t: Tab| {
        if tab() == t {
            "tab-page"
        } else {
            "tab-page hidden"
        }
    };
    let tab_button = move |t: Tab, glyph: Glyph, label: &'static str| {
        rsx! {
            button {
                "aria-current": if tab() == t { "page" } else { "false" },
                onclick: move |_| tab.set(t),
                Icon { glyph, size: 24 }
                span { "{label}" }
            }
        }
    };

    rsx! {
        document::Meta { name: "viewport", content: "width=device-width, initial-scale=1, viewport-fit=cover" }
        style { "{FONTS_CSS}" }
        style { "{rolx_ui::CSS}" }
        style { "{CSS}" }
        main { class: "compact",
            header {
                div { class: "brand",
                    h1 { "RolX" }
                    if let Some(name) = user() {
                        span { class: "user", "{name}" }
                    }
                }
                div { class: "header-tools",
                    {net_status(online, stamper)}
                    button {
                        class: "icon",
                        title: "Settings",
                        "aria-label": "Settings",
                        onclick: move |_| show_settings.set(true),
                        Icon { glyph: Glyph::Settings, size: 22 }
                    }
                }
            }
            if let Some(e) = notice() {
                p { class: "error", role: "alert", "{e}" }
            }
            div { class: tab_class(Tab::Stamp),
                StampPanel { selected, away, on_favourite, on_reload }
            }
            div { class: tab_class(Tab::Day),
                if logged_in {
                    nav { class: "day-nav", "aria-label": "Day",
                        button {
                            class: "icon",
                            title: "Previous day",
                            "aria-label": "Previous day",
                            onclick: move |_| day.set(day() - Duration::days(1)),
                            Icon { glyph: Glyph::ChevronLeft, size: 20 }
                        }
                        span { class: "date", "{date_label}" }
                        button {
                            class: "icon",
                            title: "Next day",
                            "aria-label": "Next day",
                            onclick: move |_| day.set(day() + Duration::days(1)),
                            Icon { glyph: Glyph::ChevronRight, size: 20 }
                        }
                        button {
                            class: "secondary",
                            onclick: move |_| day.set(Local::now().date_naive()),
                            "Today"
                        }
                        button {
                            class: "icon",
                            title: "Reload from the server",
                            "aria-label": "Reload from the server",
                            onclick: move |_| refresh += 1,
                            Icon { glyph: Glyph::RefreshCw, size: 18 }
                        }
                        button {
                            class: "icon",
                            title: "Share the day",
                            "aria-label": "Share the day",
                            onclick: share_day,
                            Icon { glyph: Glyph::Share, size: 18 }
                        }
                    }
                    UndoLine { undo }
                    if let Some(text) = info() {
                        p { class: "status-line", role: "status", "{text}" }
                    }
                }
                {day_content}
            }
            div { class: tab_class(Tab::Week),
                if logged_in {
                    WeekPanel {
                        day,
                        refresh,
                        state: stamp_state,
                        show_chart,
                        on_open_day: move |d| {
                            day.set(d);
                            tab.set(Tab::Day);
                        },
                    }
                    div { class: "controls",
                        button { class: "secondary", onclick: share_week, "Share the week" }
                    }
                } else {
                    {login_card(login)}
                }
            }
            if show_settings() {
                SettingsSheet {
                    show_chart,
                    lock,
                    lock_available,
                    user: user(),
                    confirm_logout,
                    on_close: move |_| {
                        confirm_logout.set(false);
                        show_settings.set(false);
                    },
                    on_logout,
                }
            }
        }
        nav { class: "tabbar", "aria-label": "Sections",
            {tab_button(Tab::Stamp, Glyph::Timer, "Stamp")}
            {tab_button(Tab::Day, Glyph::CalendarDays, "Day")}
            {tab_button(Tab::Week, Glyph::ChartColumn, "Week")}
        }
    }
}

/// "Offline" and what is waiting to upload, on every tab; tap to upload now.
fn net_status(online: Signal<bool>, stamper: Stamper) -> Element {
    let waiting = stamper.state.read().pending.len();
    let syncing = (stamper.syncing)();
    let text = match (online(), waiting) {
        (false, 0) => "Offline".to_string(),
        (false, n) => format!("Offline · {n} to upload"),
        (true, 0) => return rsx! {},
        (true, _) if syncing => "Uploading…".to_string(),
        (true, n) => format!("{n} to upload"),
    };
    rsx! {
        button {
            class: if online() { "net" } else { "net offline" },
            disabled: syncing || !online(),
            title: "Upload now",
            onclick: move |_| stamper.sync(false),
            "{text}"
        }
    }
}

#[component]
fn SettingsSheet(
    mut show_chart: Signal<bool>,
    lock: AppLock,
    lock_available: bool,
    user: Option<String>,
    mut confirm_logout: Signal<bool>,
    on_close: EventHandler<()>,
    on_logout: EventHandler<()>,
) -> Element {
    let server = rolx_ui::config()
        .map(|c| c.server_url)
        .unwrap_or_else(|e| e.to_string());
    // Read when the sheet opens, so it is current after a visit to the system settings.
    let live_updates = native::live_updates_allowed();
    let mut live_update_error = use_signal(|| None::<String>);
    rsx! {
        div {
            class: "sheet settings",
            role: "dialog",
            "aria-modal": "true",
            "aria-labelledby": "settings-title",
            div { class: "sheet-head",
                h2 { id: "settings-title", "Settings" }
                button { class: "secondary", onclick: move |_| on_close.call(()), "Done" }
            }
            label { class: "setting",
                span { "Show the week chart" }
                input {
                    r#type: "checkbox",
                    checked: show_chart(),
                    onchange: move |e| show_chart.set(e.checked()),
                }
            }
            // Keyed by the refused attempts: a ticked checkbox whose prompt was cancelled is drawn anew, unticked.
            for refused in [(lock.refused)()] {
                label { key: "{refused}", class: "setting",
                    span { "Lock with fingerprint or screen lock" }
                    input {
                        r#type: "checkbox",
                        checked: (lock.enabled)(),
                        disabled: !lock_available || (lock.prompting)(),
                        onchange: move |e| lock.set_enabled(e.checked()),
                    }
                }
            }
            if !lock_available {
                p { class: "muted", "Set up a fingerprint, face or screen lock on the phone to use the app lock." }
            } else if let Some(e) = (lock.error)() {
                p { class: "error", role: "alert", "{e}" }
            }
            if let Some(allowed) = live_updates {
                div { class: "setting",
                    span {
                        "Timer in the status bar"
                        span { class: "muted",
                            if allowed { " · on" } else { " · off" }
                        }
                    }
                    button {
                        class: "secondary",
                        onclick: move |_| {
                            if let Err(e) = native::open_live_update_settings() {
                                live_update_error.set(Some(format!("Could not open the settings: {e}")));
                            }
                        },
                        "Change…"
                    }
                }
                if let Some(e) = live_update_error() {
                    p { class: "error", role: "alert", "{e}" }
                }
            }
            div { class: "setting account",
                if let Some(name) = user {
                    span { "Logged in as {name}" }
                    if confirm_logout() {
                        div { class: "controls",
                            button { class: "danger", onclick: move |_| on_logout.call(()), "Log out" }
                            button { class: "secondary", onclick: move |_| confirm_logout.set(false), "Cancel" }
                        }
                        p { class: "muted", "Stamps that are not uploaded yet stay on the phone." }
                    } else {
                        button { class: "secondary", onclick: move |_| confirm_logout.set(true), "Log out…" }
                    }
                } else {
                    span { class: "muted", "Not logged in. Log in on the Day or Week tab." }
                }
            }
            p { class: "muted", "Server: {server}" }
        }
    }
}
