use super::App;
use chrono::{Duration, Local, NaiveDate};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

impl App {
    pub(crate) fn move_day(&mut self, day: NaiveDate) {
        self.day = day;
        self.record = None;
        self.sel_entry = 0;
        self.load_day();
    }

    pub(crate) fn on_day_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left | KeyCode::Char('[') => self.move_day(self.day - Duration::days(1)),
            KeyCode::Right | KeyCode::Char(']') => self.move_day(self.day + Duration::days(1)),
            KeyCode::Char('t') => self.move_day(Local::now().date_naive()),
            KeyCode::Up | KeyCode::Char('k') => self.scroll(-1),
            KeyCode::Down | KeyCode::Char('j') => self.scroll(1),
            KeyCode::Char('l') | KeyCode::Char('e') | KeyCode::Enter => {
                self.open_edit(Some(self.sel_entry));
            }
            KeyCode::Char('n' | 'o') => self.open_edit(None),
            KeyCode::Char('d') => self.pending = "d".into(),
            _ => {}
        }
    }

    pub(crate) fn can_edit(&mut self) -> bool {
        if self.record.is_none() {
            self.status = Some("Nothing loaded to edit".into());
        } else if self.is_locked() {
            self.status = Some("Day is before the edit lock and read-only".into());
        } else {
            return true;
        }
        false
    }

    pub(crate) fn delete_entry(&mut self) {
        if !self.can_edit() {
            return;
        }
        let Some(mut record) = self.record.clone() else {
            return;
        };
        if self.sel_entry >= record.entries.len() {
            return;
        }
        record.entries.remove(self.sel_entry);
        self.put(record);
    }
}
