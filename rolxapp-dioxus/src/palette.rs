//! Command palette (Ctrl+K): type to start working on an activity or to run a command, without
//! touching the mouse.

use chrono::{Duration, NaiveDate};
use dioxus::prelude::*;
use rolx_core::activities::search;
use rolx_ui::lucide::{Glyph, Icon};
use rolx_ui::stamper::Stamper;

const MAX_ACTIVITIES: usize = 30;

#[derive(Clone, Copy, PartialEq)]
enum Cmd {
    StampOut,
    Break,
    Resume,
    Today,
    Previous,
    Next,
    Sync,
    Reload,
    Undo,
}

#[derive(Clone, Copy, PartialEq)]
enum Item {
    Command(Cmd),
    Activity(i32),
}

fn matches_all(label: &str, query: &str) -> bool {
    let label = label.to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|word| label.contains(word))
}

#[component]
pub fn Palette(
    open: Signal<bool>,
    mut day: Signal<NaiveDate>,
    on_undo: EventHandler<()>,
    on_reload: EventHandler<()>,
) -> Element {
    let stamper = use_context::<Stamper>();
    let mut query = use_signal(String::new);
    let mut cursor = use_signal(|| 0usize);

    use_effect(move || {
        cursor();
        document::eval(
            "document.querySelector('.palette .opt.cur')?.scrollIntoView({block: 'nearest'})",
        );
    });
    let state = stamper.state.read();
    let stamped_in = state.open.is_some();
    let on_break = state
        .open
        .as_ref()
        .is_some_and(|o| o.break_started.is_some());

    let mut commands: Vec<(Cmd, &str)> = Vec::new();
    if stamped_in {
        commands.push((Cmd::StampOut, "Stamp out"));
        if on_break {
            commands.push((Cmd::Resume, "Resume work after the break"));
        } else {
            commands.push((Cmd::Break, "Start a break"));
        }
    }
    commands.extend([
        (Cmd::Today, "Go to today"),
        (Cmd::Previous, "Go to the previous day"),
        (Cmd::Next, "Go to the next day"),
        (Cmd::Sync, "Sync now"),
        (Cmd::Reload, "Reload the activity list"),
        (Cmd::Undo, "Undo the last change"),
    ]);
    let commands: Vec<(Cmd, &str)> = commands
        .into_iter()
        .filter(|(_, label)| matches_all(label, &query()))
        .collect();

    let all = stamper.activities.read();
    let verb = if stamped_in { "Switch to" } else { "Start" };
    let activity_rows: Vec<(i32, String, String)> = search(&all, &state.usage, &query())
        .into_iter()
        .take(MAX_ACTIVITIES)
        .map(|h| (h.activity.id, h.activity.label(), h.activity.group_title()))
        .collect();

    let items: Vec<Item> = commands
        .iter()
        .map(|(c, _)| Item::Command(*c))
        .chain(activity_rows.iter().map(|(id, _, _)| Item::Activity(*id)))
        .collect();
    let command_count = commands.len();
    drop(state);
    drop(all);

    let mut run = move |item: Item| {
        open.set(false);
        match item {
            Item::Activity(id) => stamper.start(id, None),
            Item::Command(cmd) => match cmd {
                Cmd::StampOut => stamper.run(Box::new(|s, now| {
                    s.stamp_out(now)?;
                    Ok(())
                })),
                Cmd::Break => stamper.run(Box::new(|s, now| Ok(s.start_break(now)?))),
                Cmd::Resume => stamper.run(Box::new(|s, now| Ok(s.end_break(now)?))),
                Cmd::Today => day.set(chrono::Local::now().date_naive()),
                Cmd::Previous => day.set(day() - Duration::days(1)),
                Cmd::Next => day.set(day() + Duration::days(1)),
                Cmd::Sync => stamper.sync(false),
                Cmd::Reload => on_reload.call(()),
                Cmd::Undo => on_undo.call(()),
            },
        }
    };

    let keys_items = items.clone();
    rsx! {
        div { class: "backdrop dim", "aria-hidden": "true", onclick: move |_| open.set(false) }
        div { class: "palette", role: "dialog", "aria-modal": "true", "aria-label": "Quick actions",
            input {
                "aria-label": "Start an activity or run a command",
                placeholder: "Start an activity or run a command…",
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
                onkeydown: move |e: KeyboardEvent| match e.key() {
                    Key::ArrowDown => {
                        e.prevent_default();
                        cursor.set((cursor() + 1).min(keys_items.len().saturating_sub(1)));
                    }
                    Key::ArrowUp => {
                        e.prevent_default();
                        cursor.set(cursor().saturating_sub(1));
                    }
                    Key::Enter => {
                        if let Some(item) = keys_items.get(cursor()) {
                            run(*item);
                        }
                    }
                    Key::Escape => open.set(false),
                    _ => {}
                },
            }
            div { class: "options",
                if items.is_empty() {
                    p { class: "empty", "Nothing matches" }
                }
                for (i , (cmd , label)) in commands.iter().enumerate() {
                    div {
                        key: "c{i}",
                        class: if cursor() == i { "opt cur" } else { "opt" },
                        onclick: {
                            let cmd = *cmd;
                            move |_| run(Item::Command(cmd))
                        },
                        span { class: "kind", Icon { glyph: Glyph::ChevronRight } }
                        span { "{label}" }
                    }
                }
                for (i , (id , label , group)) in activity_rows.iter().enumerate() {
                    div {
                        key: "a{id}",
                        class: if cursor() == command_count + i { "opt cur" } else { "opt" },
                        onclick: {
                            let id = *id;
                            move |_| run(Item::Activity(id))
                        },
                        span { class: "kind", Icon { glyph: Glyph::Play, size: 14 } }
                        span {
                            "{verb} {label}"
                            if !group.is_empty() {
                                span { class: "muted", " · {group}" }
                            }
                        }
                    }
                }
            }
            div { class: "popover-foot",
                span { class: "muted", "Up and Down to choose, Enter to run, Esc to close" }
            }
        }
    }
}
