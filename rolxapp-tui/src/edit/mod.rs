use rolx_core::api::fmt_duration;
use rolx_core::api::records::RecordEntry;

use crate::time::{entry_end, fmt_clock};

mod keys;
mod motions;
mod save;
mod validate;
mod vim;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Activity,
    From,
    To,
    Duration,
    Comment,
}

impl Field {
    const ALL: [Field; 5] = [
        Field::Activity,
        Field::From,
        Field::To,
        Field::Duration,
        Field::Comment,
    ];

    fn step(self, forward: bool) -> Field {
        let n = Self::ALL.len();
        let i = Self::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Self::ALL[(i + if forward { 1 } else { n - 1 }) % n]
    }
}

/// Which of Duration or From/To the user edited last; that one wins on save.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    Duration,
    Range,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditMode {
    /// Moving between fields.
    Nav,
    /// Typing into a text field.
    Insert,
    /// Vim-style commands inside a text field.
    Normal,
    /// Filtering the activity list.
    Search,
}

pub struct EditForm {
    /// `None` when adding a new entry.
    pub index: Option<usize>,
    pub field: Field,
    pub mode: EditMode,
    pub activity_id: i32,
    pub activity_label: String,
    pub search: String,
    pub search_sel: usize,
    pub from: String,
    pub to: String,
    pub duration: String,
    pub basis: Basis,
    pub comment: String,
    /// Character index within the field being edited.
    pub cursor: usize,
    /// Operator keys typed so far in normal mode, e.g. `ca`.
    pub pending: String,
    /// Text and cursor before the last change.
    pub undo: Option<(String, usize)>,
}

impl EditForm {
    fn blank(
        index: Option<usize>,
        field: Field,
        mode: EditMode,
        activity_id: i32,
        activity_label: String,
    ) -> Self {
        Self {
            index,
            field,
            mode,
            activity_id,
            activity_label,
            search: String::new(),
            search_sel: 0,
            from: String::new(),
            to: String::new(),
            duration: String::new(),
            basis: Basis::Duration,
            comment: String::new(),
            cursor: 0,
            pending: String::new(),
            undo: None,
        }
    }

    /// Form for changing the entry at `index`; starts on the duration field.
    pub(crate) fn for_entry(index: usize, entry: &RecordEntry, activity_label: String) -> Self {
        Self {
            from: entry.begin.map(|b| fmt_clock(b.into())).unwrap_or_default(),
            to: entry
                .begin
                .map(|b| fmt_clock(entry_end(b, entry)))
                .unwrap_or_default(),
            duration: fmt_duration(entry.duration),
            basis: if entry.begin.is_some() {
                Basis::Range
            } else {
                Basis::Duration
            },
            comment: entry.comment.clone().unwrap_or_default(),
            ..Self::blank(
                Some(index),
                Field::Duration,
                EditMode::Nav,
                entry.activity_id,
                activity_label,
            )
        }
    }

    /// Form for a new entry; starts in the activity search.
    pub(crate) fn for_new(activity_id: i32, activity_label: String) -> Self {
        Self::blank(
            None,
            Field::Activity,
            EditMode::Search,
            activity_id,
            activity_label,
        )
    }

    /// Form for booking time on a known activity; starts typing the duration.
    pub(crate) fn for_cell(activity_id: i32, activity_label: String) -> Self {
        let mut form = Self::blank(
            None,
            Field::Duration,
            EditMode::Nav,
            activity_id,
            activity_label,
        );
        form.start_insert(usize::MAX);
        form
    }
}
