//! Taking back the last change to a day (`undo.json`), and the line that offers it.

use dioxus::prelude::*;
use rolx_core::dayedit::undo_last;
use rolx_core::undo::{UndoRecord, UndoStore};

use crate::app_state::AppState;
use crate::lucide::{Glyph, Icon};
use crate::shared;
use crate::stamper::spawn_in_app;

fn read_undo() -> Option<UndoRecord> {
    UndoStore::load().ok().and_then(|s| s.last)
}

#[derive(Clone, Copy, PartialEq)]
pub struct Undo {
    /// The change that can be undone.
    pub last: Signal<Option<UndoRecord>>,
    /// Takes the last change back and goes to its day.
    pub do_undo: Callback<()>,
    /// For `DayView::on_changed`: what can be undone has changed, and the day must be reloaded.
    pub on_changed: Callback<()>,
}

/// Sets up undo on top of the app state; call it once, in the root component.
pub fn use_undo(app: AppState) -> Undo {
    let AppState {
        mut day,
        mut refresh,
        mut notice,
        mut info,
        ..
    } = app;
    let mut last = use_signal(read_undo);

    let on_changed = use_callback(move |_| {
        info.set(None);
        last.set(read_undo());
        refresh += 1;
    });

    let do_undo = use_callback(move |_| {
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
                    last.set(read_undo());
                    day.set(record.day);
                    refresh += 1;
                }
                Err(e) => notice.set(Some(e.to_string())),
            }
        });
    });

    Undo {
        last,
        do_undo,
        on_changed,
    }
}

/// "Last change: …" with an Undo button, or nothing if there is nothing to undo.
#[component]
pub fn UndoLine(undo: Undo) -> Element {
    let line = undo.last.read().as_ref().map(|u| {
        format!(
            "Last change: {} ({}, {})",
            u.what,
            u.day.format("%a %d.%m."),
            u.at.format("%H:%M")
        )
    });
    let Some(line) = line else {
        return rsx! {};
    };
    rsx! {
        p { class: "status-line",
            span { "{line}" }
            button { class: "secondary small", onclick: move |_| undo.do_undo.call(()),
                Icon { glyph: Glyph::Undo2 }
                "Undo"
            }
        }
    }
}
