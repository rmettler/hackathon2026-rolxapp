use chrono::{Days, NaiveDate};
use dioxus::prelude::*;
use dioxus_icons::lucide::{ChevronLeft, ChevronRight};
use rolx_core::clock;

use crate::components::button::{Button, ButtonVariant};
use crate::day_model::{self, DayEntries, LoadError};
use crate::platform::PlatformHandle;
use crate::state::{AppState, RouteEvent};
use crate::views::Header;

#[component]
pub fn DayView() -> Element {
    let platform = use_context::<PlatformHandle>();
    let state = use_context::<AppState>();
    let mut date = use_signal(clock::today);

    let mut day = use_resource(move || {
        let platform = platform.clone();
        let date = date();
        async move { load(&platform, state, date).await }
    });

    rsx! {
        main { class: "flex flex-col h-screen box-border text-slate-900",
            Header { title: "RolX" }
            div { class: "flex flex-col flex-1 min-h-0 gap-4 p-8",
            div { class: "flex flex-row items-center gap-2",
                Button {
                    id: "previous-day",
                    variant: ButtonVariant::Outline,
                    "aria-label": "Previous day",
                    onclick: move |_| date.set(shifted(date(), -1)),
                    ChevronLeft { size: "16px", stroke: "#0369a1" }
                }
                h1 { id: "date", class: "text-2xl font-bold text-sky-700", "{date}" }
                Button {
                    id: "next-day",
                    variant: ButtonVariant::Outline,
                    "aria-label": "Next day",
                    onclick: move |_| date.set(shifted(date(), 1)),
                    ChevronRight { size: "16px", stroke: "#0369a1" }
                }
                Button {
                    id: "today",
                    variant: ButtonVariant::Ghost,
                    onclick: move |_| date.set(clock::today()),
                    "Today"
                }
            }
            // Checked first: while loading, the resource still holds the result of the previous day.
            match (day.pending(), &*day.read()) {
                (true, _) | (_, None) => rsx! { p { id: "loading", "Loading…" } },
                (_, Some(Err(LoadError::SessionRejected))) => rsx! {},
                (_, Some(Err(LoadError::Failed(message)))) => rsx! {
                    div { id: "error", class: "flex flex-col items-start gap-2",
                        p { class: "text-red-700", "{message}" }
                        Button {
                            variant: ButtonVariant::Outline,
                            onclick: move |_| day.restart(),
                            "Try again"
                        }
                    }
                },
                (_, Some(Ok(entries))) => rsx! { Entries { entries: entries.clone() } },
            }
            }
        }
    }
}

#[component]
pub fn Entries(entries: DayEntries) -> Element {
    if entries.rows.is_empty() {
        return rsx! { p { id: "empty", "No entries for this day." } };
    }

    rsx! {
        div { id: "entries", class: "flex flex-col flex-1 min-h-0 overflow-y-scroll",
            for (index, row) in entries.rows.iter().enumerate() {
                div {
                    key: "{index}",
                    class: "flex flex-row gap-4 p-1 hover:bg-sky-100",
                    span { class: "w-40", "{row.activity}" }
                    span { class: "w-12 text-right", "{row.duration}" }
                    span { class: "flex-1", "{row.comment}" }
                }
            }
        }
        div { id: "total", class: "flex flex-row gap-4 p-1 font-bold",
            span { class: "w-40", "Total" }
            span { class: "w-12 text-right", "{entries.total}" }
        }
    }
}

fn shifted(date: NaiveDate, days: i64) -> NaiveDate {
    let moved = if days < 0 {
        date.checked_sub_days(Days::new(days.unsigned_abs()))
    } else {
        date.checked_add_days(Days::new(days.unsigned_abs()))
    };
    moved.unwrap_or(date)
}

/// Loads a day and acts on the session: a refreshed one replaces the current one, a rejected one
/// ends it and leads to the login.
async fn load(
    platform: &PlatformHandle,
    mut state: AppState,
    date: NaiveDate,
) -> Result<DayEntries, LoadError> {
    let Some(session) = state.session.peek().clone() else {
        return Err(LoadError::SessionRejected);
    };

    let result = async {
        let session = day_model::fresh_session(platform, session).await?;
        state.session.set(Some(session.clone()));
        day_model::load_day(platform, &session, date).await
    }
    .await;

    if result == Err(LoadError::SessionRejected) {
        end_session(platform, state);
    }
    result
}

/// Ends a session the server does not accept any more. The stored one is deleted only if it is that
/// same session: the CLI shares the file and may have signed in again meanwhile.
pub fn end_session(platform: &PlatformHandle, state: AppState) {
    let rejected = state.session.peek().clone();
    let store = platform.session_store();
    let is_stored = match (store.load(), rejected) {
        (Ok(Some(stored)), Some(rejected)) => {
            stored.bearer_token.as_str() == rejected.bearer_token.as_str()
        }
        _ => false,
    };
    if is_stored && store.delete().is_err() {
        tracing::warn!("could not delete the rejected session");
    }
    state.navigate(RouteEvent::SessionRejected);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::day_model::EntryRow;

    fn render(entries: DayEntries) -> String {
        let mut dom = VirtualDom::new_with_props(Entries, EntriesProps { entries });
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    #[test]
    fn entries_show_rows_and_total() {
        let html = render(DayEntries {
            rows: vec![EntryRow {
                activity: "M0001.001.01".into(),
                duration: "1:30".into(),
                comment: "Mock".into(),
            }],
            total: "9:59".into(),
        });

        assert!(html.contains("M0001.001.01"), "{html}");
        assert!(html.contains("1:30"), "{html}");
        assert!(html.contains("Mock"), "{html}");
        assert!(html.contains("Total"), "{html}");
        assert!(html.contains("9:59"), "{html}");
    }

    #[test]
    fn no_entries_show_the_empty_state() {
        let html = render(DayEntries {
            rows: Vec::new(),
            total: "0:00".into(),
        });

        assert!(html.contains("No entries"), "{html}");
        assert!(!html.contains("Total"), "{html}");
    }

    #[test]
    fn shifting_stops_at_the_end_of_the_calendar() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        assert_eq!(
            shifted(date, -1),
            NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
        );
        assert_eq!(
            shifted(date, 1),
            NaiveDate::from_ymd_opt(2026, 3, 2).unwrap()
        );
        assert_eq!(shifted(NaiveDate::MAX, 1), NaiveDate::MAX);
    }
}
