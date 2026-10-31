//! Looking at and settling pending changes: the outbox list and the conflict dialog.

use chrono::NaiveDate;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use rolx_core::api::records::Record;

use super::{App, DayView};
use crate::outbox::{ChangeState, days_equal, merge_day};

/// The list of pending changes.
pub struct OutboxPopup {
    pub selected: usize,
    /// Whether discarding the selected change waits for confirmation.
    pub confirm_discard: bool,
}

/// The choice for a day changed both here and on the server.
pub struct ConflictDialog {
    pub date: NaiveDate,
    /// 0 merge, 1 keep mine, 2 keep theirs.
    pub choice: usize,
    /// The merged day, or why the two sides can't be merged.
    pub merged: Result<Record, String>,
}

impl App {
    /// `O`: list the pending changes.
    pub(crate) fn open_outbox(&mut self) {
        if self.outbox.is_empty() {
            self.status = Some("No pending changes".into());
        } else {
            self.outbox_popup = Some(OutboxPopup {
                selected: 0,
                confirm_discard: false,
            });
        }
    }

    pub(crate) fn on_outbox_key(&mut self, key: KeyEvent) {
        let Some(popup) = &self.outbox_popup else {
            return;
        };
        let (selected, confirming) = (popup.selected, popup.confirm_discard);
        let date = self.outbox.by_date().get(selected).map(|c| c.date);
        let last = self.outbox.len().saturating_sub(1);
        if confirming {
            self.outbox_popup = Some(OutboxPopup {
                selected,
                confirm_discard: false,
            });
            if key.code == KeyCode::Char('y')
                && let Some(date) = date
            {
                self.discard_change(date);
            }
            return;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q' | 'O') => self.outbox_popup = None,
            KeyCode::Up | KeyCode::Char('k') => {
                self.outbox_popup = Some(OutboxPopup {
                    selected: selected.saturating_sub(1),
                    confirm_discard: false,
                });
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.outbox_popup = Some(OutboxPopup {
                    selected: (selected + 1).min(last),
                    confirm_discard: false,
                });
            }
            KeyCode::Char('d') if date.is_some() => {
                self.outbox_popup = Some(OutboxPopup {
                    selected,
                    confirm_discard: true,
                });
            }
            KeyCode::Char('r') => {
                if let Some(date) = date {
                    self.retry_change(date);
                }
            }
            KeyCode::Enter => {
                if let Some(date) = date {
                    match self.outbox.get(date).map(|c| c.state) {
                        Some(ChangeState::Conflict) => self.open_conflict(date),
                        _ => self.goto_day(date),
                    }
                }
            }
            _ => {}
        }
    }

    fn goto_day(&mut self, date: NaiveDate) {
        self.outbox_popup = None;
        self.day = date;
        self.day_view = DayView::Day;
        self.set_tab(0);
        self.sel_entry = 0;
        self.record = self.outbox.get(date).map(|c| c.record.clone());
        self.load_day();
    }

    /// Takes a refused change up again, for when the cause is gone.
    fn retry_change(&mut self, date: NaiveDate) {
        match self.outbox.get_mut(date) {
            Some(change) if change.state == ChangeState::Rejected => {
                change.state = ChangeState::Pending;
                change.message = None;
            }
            Some(change) if change.state == ChangeState::Conflict => {
                self.status = Some("Press Enter to settle the conflict".into());
                return;
            }
            _ => {}
        }
        self.save_outbox_status();
        self.start_sync();
    }

    fn save_outbox_status(&mut self) {
        if let Err(error) = self.persist_outbox() {
            self.status = Some(format!("Can't save the pending changes: {error}"));
        }
    }

    /// Drops a pending change; the day is shown as the local copy holds it again.
    fn discard_change(&mut self, date: NaiveDate) {
        if self.outbox.remove(date).is_none() {
            return;
        }
        self.save_outbox_status();
        if let Some(record) = self.copy.day(date).cloned() {
            self.show(&record);
        }
        self.load_view();
        match self.outbox.len() {
            0 => self.outbox_popup = None,
            n => {
                if let Some(popup) = &mut self.outbox_popup {
                    popup.selected = popup.selected.min(n - 1);
                }
            }
        }
        self.status = Some("Pending change discarded".into());
    }

    pub(crate) fn open_conflict(&mut self, date: NaiveDate) {
        let Some(change) = self.outbox.get(date) else {
            return;
        };
        let Some(server) = &change.server else {
            return;
        };
        let merged = merge_day(&change.base, &change.record, server);
        self.conflict = Some(ConflictDialog {
            date,
            choice: if merged.is_ok() { 0 } else { 1 },
            merged,
        });
    }

    pub(crate) fn on_conflict_key(&mut self, key: KeyEvent) {
        let Some(dialog) = &mut self.conflict else {
            return;
        };
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.conflict = None,
            KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => {
                dialog.choice = dialog.choice.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                dialog.choice = (dialog.choice + 1).min(2);
            }
            KeyCode::Char(c @ '1'..='3') => dialog.choice = c as usize - '1' as usize,
            KeyCode::Enter => {
                let choice = dialog.choice;
                self.resolve(choice);
            }
            _ => {}
        }
    }

    /// Settles the open conflict: merge, keep mine or keep theirs.
    fn resolve(&mut self, choice: usize) {
        let Some(dialog) = &self.conflict else {
            return;
        };
        let date = dialog.date;
        let Some(change) = self.outbox.get(date).cloned() else {
            self.conflict = None;
            return;
        };
        let Some(server) = change.server.clone() else {
            self.conflict = None;
            return;
        };
        let wanted = match choice {
            0 => match &dialog.merged {
                Ok(merged) => merged.clone(),
                Err(reason) => {
                    self.status = Some(format!("Can't merge: {reason}"));
                    return;
                }
            },
            1 => change.record.clone(),
            _ => server.clone(),
        };
        self.conflict = None;

        // The server's day is now what the copy holds and what the change builds on.
        let months = self.copy.put_days(std::slice::from_ref(&server));
        self.persist_months(months);
        let keep_change = choice != 2 && !days_equal(&wanted, &server);
        if keep_change {
            if let Some(change) = self.outbox.get_mut(date) {
                change.base = server;
                change.record = wanted.clone();
                change.state = ChangeState::Pending;
                change.server = None;
                change.message = None;
            }
        } else {
            self.outbox.remove(date);
        }
        self.save_outbox_status();
        self.show(&wanted);
        if self.outbox.is_empty() {
            self.outbox_popup = None;
        }
        self.status = Some(
            match (choice, keep_change) {
                (2, _) => "Kept the server's day",
                (_, false) => "Nothing left to upload",
                _ => "Resolved, will upload",
            }
            .into(),
        );
        self.start_sync();
    }
}
