use dioxus::prelude::*;
use rolx_core::activities::{Activity, Group, search};
use rolx_core::stamp::StampState;

use crate::lucide::{Glyph, Icon};

/// Rows rendered at most; more than that cannot be scanned by eye anyway, so keep typing instead.
const MAX_ROWS: usize = 100;
const CHIPS: usize = 5;

fn heading(group: Group, activity: &Activity) -> String {
    match group {
        Group::Favourite => "Favourites".into(),
        Group::Recent => "Recently used".into(),
        Group::Other => match activity.group_title() {
            t if t.is_empty() => "All activities".into(),
            t => t,
        },
    }
}

/// Searchable activity selector: favourites and recently used first, type to filter, arrow keys
/// and Enter to choose, click the star to pin a favourite.
#[component]
pub fn ActivityPicker(
    activities: Signal<Vec<Activity>>,
    state: Signal<StampState>,
    selected: Signal<Option<i32>>,
    on_favourite: EventHandler<(i32, bool)>,
    on_reload: EventHandler<()>,
    #[props(default = true)] show_chips: bool,
) -> Element {
    let mut open = use_signal(|| false);
    let mut query = use_signal(String::new);
    let mut cursor = use_signal(|| 0usize);

    use_effect(move || {
        cursor();
        open();
        document::eval(
            "document.querySelector('.picker .opt.cur')?.scrollIntoView({block: 'nearest'})",
        );
    });

    let all = activities.read();
    let current = state.read();

    let chips: Vec<Activity> = search(&all, &current.usage, "")
        .into_iter()
        .filter(|h| h.group != Group::Other)
        .take(CHIPS)
        .map(|h| h.activity.clone())
        .collect();

    let hits = search(&all, &current.usage, &query());
    let total = hits.len();
    let shown: Vec<(Group, Activity)> = hits
        .into_iter()
        .take(MAX_ROWS)
        .map(|h| (h.group, h.activity.clone()))
        .collect();
    let ids: Vec<i32> = shown.iter().map(|(_, a)| a.id).collect();

    let trigger_label = selected()
        .and_then(|id| all.iter().find(|a| a.id == id))
        .map(Activity::label);
    let none_available = all.is_empty();

    let mut last_heading = String::new();
    let rows: Vec<(Option<String>, usize, Activity)> = shown
        .into_iter()
        .enumerate()
        .map(|(i, (group, a))| {
            let h = heading(group, &a);
            let new_heading = (h != last_heading).then(|| h.clone());
            last_heading = h;
            (new_heading, i, a)
        })
        .collect();

    rsx! {
        div { class: "picker",
            if show_chips && !chips.is_empty() {
                div { class: "chips",
                    for chip in chips {
                        button {
                            key: "{chip.id}",
                            class: if selected() == Some(chip.id) { "chip on" } else { "chip" },
                            title: "{chip.label()}",
                            onclick: {
                                let id = chip.id;
                                move |_| selected.set(if selected() == Some(id) { None } else { Some(id) })
                            },
                            if chip.favourite {
                                Icon { glyph: Glyph::Star, size: 14, filled: true }
                            }
                            "{chip.short_label()}"
                        }
                    }
                }
            }
            div { class: "trigger-row",
                button {
                    class: "secondary trigger",
                    disabled: none_available,
                    onclick: move |_| {
                        open.set(!open());
                        query.set(String::new());
                        cursor.set(0);
                    },
                    span { class: "trigger-label",
                        if none_available {
                            "No activities yet. Log in and sync"
                        } else if let Some(label) = &trigger_label {
                            "{label}"
                        } else {
                            "Choose activity ({all.len()})"
                        }
                    }
                    Icon { glyph: Glyph::ChevronDown }
                }
                if selected().is_some() {
                    button {
                        class: "icon",
                        title: "No activity",
                        "aria-label": "Clear activity",
                        onclick: move |_| selected.set(None),
                        Icon { glyph: Glyph::X }
                    }
                }
            }
            if open() {
                div { class: "backdrop", onclick: move |_| open.set(false) }
                div { class: "popover",
                    input {
                        placeholder: "Search number, name, customer, project…",
                        value: "{query}",
                        onmounted: move |e| {
                            spawn(async move {
                                _ = e.set_focus(true).await;
                            });
                        },
                        oninput: move |e| {
                            query.set(e.value());
                            cursor.set(0);
                        },
                        onkeydown: {
                            let ids = ids.clone();
                            move |e: KeyboardEvent| match e.key() {
                                Key::ArrowDown => {
                                    e.prevent_default();
                                    cursor.set((cursor() + 1).min(ids.len().saturating_sub(1)));
                                }
                                Key::ArrowUp => {
                                    e.prevent_default();
                                    cursor.set(cursor().saturating_sub(1));
                                }
                                Key::Enter => {
                                    if let Some(id) = ids.get(cursor()) {
                                        selected.set(Some(*id));
                                        open.set(false);
                                    }
                                }
                                Key::Escape => open.set(false),
                                _ => {}
                            }
                        },
                    }
                    div { class: "options",
                        if rows.is_empty() {
                            p { class: "empty", "No activity matches" }
                        }
                        for (new_heading, i, a) in rows {
                            div { key: "{a.id}",
                                if let Some(h) = new_heading {
                                    div { class: "group", "{h}" }
                                }
                                div {
                                    class: if cursor() == i { "opt cur" } else if selected() == Some(a.id) { "opt sel" } else { "opt" },
                                    onclick: {
                                        let id = a.id;
                                        move |_| {
                                            selected.set(Some(id));
                                            open.set(false);
                                        }
                                    },
                                    button {
                                        class: if a.favourite { "star on" } else { "star" },
                                        title: if a.favourite { "Remove from favourites" } else { "Add to favourites" },
                                        "aria-label": if a.favourite { "Remove from favourites" } else { "Add to favourites" },
                                        onclick: {
                                            let id = a.id;
                                            let fav = a.favourite;
                                            move |e: MouseEvent| {
                                                e.stop_propagation();
                                                on_favourite.call((id, !fav));
                                            }
                                        },
                                        Icon { glyph: Glyph::Star, filled: a.favourite }
                                    }
                                    span { "{a.label()}" }
                                }
                            }
                        }
                        if total > MAX_ROWS {
                            p { class: "empty", "{total - MAX_ROWS} more. Type to narrow down" }
                        }
                    }
                    div { class: "popover-foot",
                        span { class: "muted", "{all.len()} activities" }
                        button {
                            class: "secondary",
                            title: "Download the full list from the server",
                            onclick: move |_| on_reload.call(()),
                            Icon { glyph: Glyph::RefreshCw }
                            "Reload list"
                        }
                    }
                }
            }
        }
    }
}
