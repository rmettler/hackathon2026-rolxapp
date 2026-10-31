use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration as StdDuration;

use chrono::{Duration, Local, NaiveDate};
use dioxus::desktop::tao::dpi::{PhysicalPosition, PhysicalSize};
use dioxus::desktop::tao::window::Icon as WindowIcon;
use dioxus::desktop::trayicon::menu::{Menu, MenuItem, PredefinedMenuItem};
use dioxus::desktop::trayicon::{Icon as TrayIcon, MouseButton, MouseButtonState, TrayIconEvent};
use dioxus::desktop::{
    Config, HotKeyState, LogicalPosition, LogicalSize, WindowBuilder, WindowCloseBehaviour, window,
};
use dioxus::prelude::*;
use rolx_core::activities::{self, Activity};
use rolx_core::config::AppConfig;
use rolx_core::dayedit::{load_day as load_day_entries, undo_last};
use rolx_core::dayplan::DayEntry;
use rolx_core::idle::{Away, IdleTracker};
use rolx_core::login::{self, AuthFlow};
use rolx_core::stamp::StampState;
use rolx_core::undo::{UndoRecord, UndoStore};
use tokio::sync::mpsc;

mod activity_picker;
mod day_view;
mod fonts;
mod icon;
mod lucide;
mod os_idle;
mod palette;
mod settings;
mod shared;
mod stamp_panel;
mod stamper;
mod week_panel;
use day_view::DayView;
use lucide::{Glyph, Icon};
use palette::Palette;
use settings::{Settings, WindowGeometry};
use stamp_panel::StampPanel;
use stamper::{Stamper, spawn_in_app};
use week_panel::WeekPanel;

const TOKENS: &str = include_str!("tokens.css");
const CSS: &str = include_str!("style.css");

// Window defaults (logical pixels), used when no position and size were saved: the first start
// opens maximized. The position is measured from the top-left corner of the primary screen.
const WINDOW_SIZE: (f64, f64) = (1280.0, 800.0);
const WINDOW_MIN_SIZE: (f64, f64) = (480.0, 400.0);
const WINDOW_POSITION: (f64, f64) = (60.0, 40.0);

const ICON_SIZE: u32 = 64;

fn main() {
    rolx_core::logging::init(0);
    let settings = Settings::load();
    let icon = WindowIcon::from_rgba(icon::rgba(ICON_SIZE), ICON_SIZE, ICON_SIZE).ok();
    let saved_window = settings.window.is_some();
    let window = WindowBuilder::new()
        .with_title("RolX")
        .with_window_icon(icon)
        .with_inner_size(LogicalSize::new(WINDOW_SIZE.0, WINDOW_SIZE.1))
        .with_min_inner_size(LogicalSize::new(WINDOW_MIN_SIZE.0, WINDOW_MIN_SIZE.1))
        .with_position(LogicalPosition::new(WINDOW_POSITION.0, WINDOW_POSITION.1))
        .with_maximized(!saved_window);
    let close = if settings.close_to_tray {
        WindowCloseBehaviour::WindowHides
    } else {
        WindowCloseBehaviour::WindowCloses
    };
    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            Config::new()
                .with_window(window)
                .with_close_behaviour(close),
        )
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

async fn log_in() -> anyhow::Result<()> {
    let config = AppConfig::load_with_env()?;
    login::run(&config, AuthFlow::NativeApp).await?;
    Ok(())
}

/// What the tray menu, the global hotkey and the keyboard shortcuts ask the app to do.
#[derive(Debug, Clone, Copy)]
enum Request {
    ShowWindow,
    ToggleStamp,
    Quit,
    Palette,
    PreviousDay,
    NextDay,
    Today,
}

type Requests = (
    mpsc::UnboundedSender<Request>,
    Rc<RefCell<Option<mpsc::UnboundedReceiver<Request>>>>,
);

const KEY_LISTENER: &str = r#"
window.addEventListener('keydown', e => {
  const key = e.key.toLowerCase();
  const plain = !e.shiftKey;
  if ((e.ctrlKey || e.metaKey) && !e.altKey && plain && key === 'k') {
    e.preventDefault(); dioxus.send('palette');
  } else if (e.altKey && !e.ctrlKey && plain && e.key === 'ArrowLeft') {
    e.preventDefault(); dioxus.send('previous');
  } else if (e.altKey && !e.ctrlKey && plain && e.key === 'ArrowRight') {
    e.preventDefault(); dioxus.send('next');
  } else if (e.altKey && !e.ctrlKey && plain && key === 't') {
    e.preventDefault(); dioxus.send('today');
  }
}, true);
"#;

fn show_window() {
    let w = window();
    w.set_visible(true);
    w.set_minimized(false);
    w.set_focus();
}

fn tray_menu() -> Menu {
    let menu = Menu::new();
    let _ = menu.append_items(&[
        &MenuItem::with_id("show", "Show RolX", true, None),
        &MenuItem::with_id("toggle", "Stamp in / out", true, None),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id("quit", "Quit RolX", true, None),
    ]);
    menu
}

fn idle_threshold(settings: &Settings) -> Duration {
    // For trying it out without waiting minutes.
    match std::env::var("ROLX_IDLE_SECS")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
    {
        Some(secs) => Duration::seconds(secs),
        None => Duration::minutes(settings.idle_minutes),
    }
}

/// Puts the window where it was last seen, if that is still on a screen.
fn restore_window(saved: Option<WindowGeometry>) {
    let Some(g) = saved else {
        return;
    };
    let w = window();
    let (right, bottom) = (g.x + g.width as i32, g.y + g.height as i32);
    // A part of the title bar must be on some screen, so the window can always be grabbed.
    let reachable = w.available_monitors().any(|m| {
        let (p, s) = (m.position(), m.size());
        g.x + 100 < p.x + s.width as i32
            && right - 100 > p.x
            && g.y >= p.y
            && g.y + 40 < p.y + s.height as i32
    });
    if !reachable || bottom <= 0 {
        return;
    }
    w.set_inner_size(PhysicalSize::new(g.width, g.height));
    w.set_outer_position(PhysicalPosition::new(g.x, g.y));
    if g.maximized {
        w.set_maximized(true);
    }
}

fn current_geometry(previous: Option<WindowGeometry>) -> Option<WindowGeometry> {
    let w = window();
    if w.is_minimized() || !w.is_visible() {
        return None;
    }
    if w.is_maximized() {
        return previous.map(|p| WindowGeometry {
            maximized: true,
            ..p
        });
    }
    let position = w.outer_position().ok()?;
    let size = w.inner_size();
    // Windows reports far-off positions for windows that are being minimized.
    (position.x > -10_000 && size.width > 0).then_some(WindowGeometry {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        maximized: false,
    })
}

fn read_undo() -> Option<UndoRecord> {
    UndoStore::load().ok().and_then(|s| s.last)
}

#[component]
fn App() -> Element {
    let mut day = use_signal(|| Local::now().date_naive());
    let mut refresh = use_signal(|| 0u32);
    let mut login_busy = use_signal(|| false);
    let mut notice = use_signal(|| None::<String>);
    let mut info = use_signal(|| None::<String>);
    let mut palette_open = use_signal(|| false);
    let mut away = use_signal(|| None::<Away>);
    let mut undo = use_signal(read_undo);

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
    };

    let on_changed = move |_| {
        info.set(None);
        undo.set(read_undo());
        refresh += 1;
    };

    let do_undo = move || {
        spawn_in_app(async move {
            let result = async {
                let client = shared::client().await?;
                undo_last(&client).await
            }
            .await;
            match result {
                Ok(record) => {
                    notice.set(None);
                    info.set(Some(format!("Undone: {}", record.what)));
                    undo.set(read_undo());
                    day.set(record.day);
                    refresh += 1;
                }
                Err(e) => notice.set(Some(e.to_string())),
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

    // Requests from places that cannot touch signals directly (tray, hotkey, the web view) are
    // queued here and handled in the app.
    let (requests, receiver): Requests = use_hook(|| {
        let (tx, rx) = mpsc::unbounded_channel();
        (tx, Rc::new(RefCell::new(Some(rx))))
    });
    let settings = use_hook(Settings::load);

    let tray = use_hook(|| {
        let icon = TrayIcon::from_rgba(icon::rgba(32), 32, 32).ok();
        dioxus::desktop::trayicon::init_tray_icon(tray_menu(), icon)
    });
    use_tray_events(requests.clone());

    if !settings.hotkey.is_empty() {
        let tx = requests.clone();
        let registered =
            dioxus::desktop::use_global_shortcut(settings.hotkey.as_str(), move |state| {
                if state == HotKeyState::Pressed {
                    let _ = tx.send(Request::ToggleStamp);
                }
            });
        if let Err(e) = registered {
            // The result is cached by the hook, so this would print on every render.
            static REPORTED: std::sync::Once = std::sync::Once::new();
            REPORTED.call_once(|| {
                eprintln!("cannot register the hotkey '{}': {e:?}", settings.hotkey);
            });
        }
    }

    use_hook(|| restore_window(settings.window));

    use_future({
        let receiver = receiver.clone();
        move || {
            let receiver = receiver.clone();
            async move {
                let Some(mut rx) = receiver.borrow_mut().take() else {
                    return;
                };
                while let Some(request) = rx.recv().await {
                    match request {
                        Request::ShowWindow => show_window(),
                        Request::ToggleStamp => stamper.toggle(),
                        Request::Quit => std::process::exit(0),
                        Request::Palette => {
                            show_window();
                            palette_open.set(!palette_open());
                        }
                        Request::PreviousDay => day.set(day() - Duration::days(1)),
                        Request::NextDay => day.set(day() + Duration::days(1)),
                        Request::Today => day.set(Local::now().date_naive()),
                    }
                }
            }
        }
    });

    use_future({
        let tx = requests.clone();
        move || {
            let tx = tx.clone();
            async move {
                let mut eval = document::eval(KEY_LISTENER);
                while let Ok(key) = eval.recv::<String>().await {
                    let request = match key.as_str() {
                        "palette" => Request::Palette,
                        "previous" => Request::PreviousDay,
                        "next" => Request::NextDay,
                        "today" => Request::Today,
                        _ => continue,
                    };
                    let _ = tx.send(request);
                }
            }
        }
    });

    // The tray tooltip shows what is running, so a glance is enough.
    use_future(move || {
        let tray = tray.clone();
        async move {
            loop {
                let text = {
                    let state = stamp_state.read();
                    match state.status(Local::now().naive_local()) {
                        rolx_core::stamp::Status::StampedOut => "RolX: stamped out".to_string(),
                        rolx_core::stamp::Status::StampedIn {
                            activity,
                            worked,
                            on_break,
                            ..
                        } => format!(
                            "RolX: {}:{:02} worked{}{}",
                            worked.num_hours(),
                            worked.num_minutes() % 60,
                            if on_break { " (on break)" } else { "" },
                            activity
                                .map(|a| format!(" on {}", a.full_number))
                                .unwrap_or_default()
                        ),
                    }
                };
                let _ = tray.set_tooltip(Some(text));
                tokio::time::sleep(StdDuration::from_secs(15)).await;
            }
        }
    });

    // Away detection: after a long stretch without input while stamped in, ask what to do with
    // that time once the user is back.
    use_future({
        let threshold = idle_threshold(&settings);
        move || async move {
            let mut tracker = IdleTracker::new(threshold);
            loop {
                tokio::time::sleep(StdDuration::from_secs(3)).await;
                let Some(idle) = os_idle::idle_time() else {
                    continue;
                };
                let tracking = {
                    let state = stamp_state.read();
                    state
                        .open
                        .as_ref()
                        .is_some_and(|o| o.break_started.is_none())
                } && away.peek().is_none();
                let idle = Duration::from_std(idle).unwrap_or_default();
                if let Some(absence) = tracker.observe(Local::now().naive_local(), idle, tracking) {
                    // The absence cannot have begun before the session did.
                    let started = stamp_state.read().open.as_ref().map(|o| o.start);
                    let since = started.map_or(absence.since, |s| s.max(absence.since));
                    if since < absence.until {
                        away.set(Some(Away {
                            since,
                            until: absence.until,
                        }));
                        show_window();
                    }
                }
            }
        }
    });

    // Remember where the window is, so it opens there next time.
    use_future(move || async move {
        let mut saved = Settings::load().window;
        loop {
            tokio::time::sleep(StdDuration::from_secs(3)).await;
            if let Some(now) = current_geometry(saved)
                && Some(now) != saved
            {
                saved = Some(now);
                Settings::update(|s| s.window = Some(now));
            }
        }
    });

    let on_login = move |_| {
        spawn(async move {
            login_busy.set(true);
            notice.set(None);
            if let Err(e) = log_in().await {
                notice.set(Some(e.to_string()));
            }
            login_busy.set(false);
            refresh += 1;
        });
    };

    let on_logout = move |_| {
        if let Err(e) = rolx_core::session::Session::delete() {
            notice.set(Some(e.to_string()));
        }
        refresh += 1;
    };

    let content = match &*loaded.read() {
        None => rsx! {
            p { class: "empty", "Loading…" }
        },
        Some(Err(e)) => rsx! {
            p { class: "error", "{e}" }
        },
        Some(Ok(Loaded::NeedsLogin)) => rsx! {
            div { class: "card empty",
                p { "You are not logged in." }
                button { disabled: login_busy(), onclick: on_login,
                    if login_busy() {
                        "Waiting for browser…"
                    } else {
                        "Log in with Google"
                    }
                }
            }
        },
        Some(Ok(Loaded::Day { entries, .. })) => {
            let date_label = day().format("%a %d.%m.%Y").to_string();
            rsx! {
                nav { class: "day-nav",
                    div { class: "stepper",
                        button {
                            class: "icon",
                            title: "Previous day (Alt+Left)",
                            "aria-label": "Previous day",
                            onclick: move |_| day.set(day() - Duration::days(1)),
                            Icon { glyph: Glyph::ChevronLeft, size: 20 }
                        }
                        span { class: "date", "{date_label}" }
                        button {
                            class: "icon",
                            title: "Next day (Alt+Right)",
                            "aria-label": "Next day",
                            onclick: move |_| day.set(day() + Duration::days(1)),
                            Icon { glyph: Glyph::ChevronRight, size: 20 }
                        }
                    }
                    button {
                        class: "secondary",
                        title: "Alt+T",
                        onclick: move |_| day.set(Local::now().date_naive()),
                        "Today"
                    }
                    button {
                        class: "secondary",
                        title: "Reload from the server",
                        onclick: move |_| refresh += 1,
                        Icon { glyph: Glyph::RefreshCw }
                        "Refresh"
                    }
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
                        on_changed,
                    }
                }
            }
        }
    };

    let user_name = match &*loaded.read() {
        Some(Ok(Loaded::Day { user, .. })) => Some(user.clone()),
        _ => None,
    };
    let logged_in = user_name.is_some();
    let undo_line = undo().map(|u| {
        format!(
            "Last change: {} ({}, {})",
            u.what,
            u.day.format("%a %d.%m."),
            u.at.format("%H:%M")
        )
    });

    let brand_css = use_hook(|| format!("{}{TOKENS}{CSS}", fonts::font_face_css()));

    rsx! {
        style { "{brand_css}" }
        main {
            header {
                div { class: "brand",
                    h1 { "RolX" }
                    if let Some(name) = user_name {
                        span { class: "user", "{name}" }
                    }
                }
                div { class: "header-tools",
                    button {
                        class: "secondary",
                        title: "Ctrl+K",
                        onclick: move |_| palette_open.set(true),
                        Icon { glyph: Glyph::Command }
                        "Quick actions"
                    }
                    if logged_in {
                        button { class: "secondary", onclick: on_logout, "Log out" }
                    }
                }
            }
            if let Some(e) = notice() {
                p { class: "error", "{e}" }
            }
            if let Some(line) = undo_line {
                p { class: "status-line",
                    span { "{line}" }
                    button { class: "secondary small", onclick: move |_| do_undo(),
                        Icon { glyph: Glyph::Undo2 }
                        "Undo"
                    }
                }
            }
            if let Some(text) = info() {
                p { class: "status-line", "{text}" }
            }
            div { class: "layout",
                aside { class: "side",
                    StampPanel { selected, away, on_favourite, on_reload }
                }
                div { class: "work", {content} }
            }
            if palette_open() {
                Palette {
                    open: palette_open,
                    day,
                    on_undo: move |_| do_undo(),
                    on_reload,
                }
            }
        }
    }
}

/// Forwards clicks on the tray icon and its menu to the app.
fn use_tray_events(tx: mpsc::UnboundedSender<Request>) {
    let menu_tx = tx.clone();
    // The menu library keeps only the first global handler, which Dioxus registers for the window
    // menu bar, so clicks in the tray menu arrive as ordinary menu events, not as tray menu events.
    dioxus::desktop::use_muda_event_handler(move |event| {
        let request = match event.id().0.as_str() {
            "show" => Request::ShowWindow,
            "toggle" => Request::ToggleStamp,
            "quit" => Request::Quit,
            _ => return,
        };
        let _ = menu_tx.send(request);
    });
    dioxus::desktop::use_tray_icon_event_handler(move |event| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            let _ = tx.send(Request::ShowWindow);
        }
    });
}
