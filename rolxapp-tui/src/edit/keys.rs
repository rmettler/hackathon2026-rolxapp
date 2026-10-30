use super::{EditForm, EditMode, Field};
use crate::app::App;
use crate::app::activities::{activity_label, matches_query};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rolx_core::api::activity::Activity;

impl App {
    pub(crate) fn open_edit(&mut self, index: Option<usize>) {
        if !self.can_edit() {
            return;
        }
        let Some(record) = &self.record else { return };
        let form = match index {
            Some(i) => {
                let Some(entry) = record.entries.get(i) else {
                    return;
                };
                let label = self
                    .find_activity(entry.activity_id)
                    .map_or_else(|| entry.full_activity_number.clone(), activity_label);
                EditForm::for_entry(i, entry, label)
            }
            None => {
                let Some(first) = self.searchable().first() else {
                    self.status = Some("No activities available".into());
                    return;
                };
                EditForm::for_new(first.id, activity_label(first))
            }
        };
        self.edit = Some(form);
    }

    pub(crate) fn on_edit_key(&mut self, key: KeyEvent) {
        let Some(mode) = self.edit.as_ref().map(|f| f.mode) else {
            return;
        };
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            self.save_edit();
            return;
        }
        match mode {
            EditMode::Nav => self.on_edit_nav_key(key),
            EditMode::Insert => {
                if let Some(form) = self.edit.as_mut() {
                    form.insert_key(key);
                }
            }
            EditMode::Normal => {
                if let Some(form) = self.edit.as_mut() {
                    form.normal_key(key);
                }
            }
            EditMode::Search => self.on_edit_search_key(key),
        }
    }

    pub(crate) fn on_edit_nav_key(&mut self, key: KeyEvent) {
        let Some(form) = self.edit.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('h') => self.edit = None,
            KeyCode::Char('s') => self.save_edit(),
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => form.field = form.field.step(true),
            KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => {
                form.field = form.field.step(false);
            }
            KeyCode::Char('l' | 'i' | 'a' | 'A' | 'I') | KeyCode::Enter => {
                if form.field == Field::Activity {
                    form.search.clear();
                    form.search_sel = 0;
                    form.mode = EditMode::Search;
                } else if matches!(key.code, KeyCode::Char('i' | 'I')) {
                    form.start_insert(0);
                } else {
                    form.start_insert(usize::MAX);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn on_edit_search_key(&mut self, key: KeyEvent) {
        let count = self.search_results().len();
        let Some(form) = self.edit.as_mut() else {
            return;
        };
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => form.mode = EditMode::Nav,
            KeyCode::Enter => self.pick_activity(),
            KeyCode::Down => form.search_sel = (form.search_sel + 1).min(count.saturating_sub(1)),
            KeyCode::Up => form.search_sel = form.search_sel.saturating_sub(1),
            KeyCode::Char('n' | 'j') if ctrl => {
                form.search_sel = (form.search_sel + 1).min(count.saturating_sub(1));
            }
            KeyCode::Char('p' | 'k') if ctrl => {
                form.search_sel = form.search_sel.saturating_sub(1);
            }
            KeyCode::Char('d') if ctrl => {
                form.search_sel = (form.search_sel + 4).min(count.saturating_sub(1));
            }
            KeyCode::Char('u') if ctrl => {
                form.search_sel = form.search_sel.saturating_sub(4);
            }
            KeyCode::Backspace => {
                form.search.pop();
                form.search_sel = 0;
            }
            KeyCode::Char(c) if !ctrl => {
                form.search.push(c);
                form.search_sel = 0;
            }
            _ => {}
        }
    }

    /// Activities matching every whitespace-separated word of the search text.
    pub fn search_results(&self) -> Vec<&Activity> {
        let Some(form) = &self.edit else {
            return Vec::new();
        };
        let query = form.search.to_lowercase();
        self.searchable()
            .iter()
            .filter(|a| matches_query(a, &query))
            .collect()
    }

    pub(crate) fn pick_activity(&mut self) {
        let Some(sel) = self.edit.as_ref().map(|f| f.search_sel) else {
            return;
        };
        let picked = self
            .search_results()
            .get(sel)
            .map(|a| (a.id, activity_label(a)));
        let Some(form) = self.edit.as_mut() else {
            return;
        };
        if let Some((id, label)) = picked {
            form.activity_id = id;
            form.activity_label = label;
            form.mode = EditMode::Nav;
        }
    }
}
