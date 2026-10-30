use super::{App, DayView};
use crate::app::activities::is_bookable;
use chrono::{Duration, Local, NaiveDate};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use rolx_core::api::activity::Activity;

impl App {
    /// Favourites that can be booked on the selected day but have no entry yet, by number.
    pub fn unbooked_favourites(&self) -> Vec<&Activity> {
        let booked = |id: i32| {
            self.record
                .as_ref()
                .is_some_and(|r| r.entries.iter().any(|e| e.activity_id == id))
        };
        let mut list: Vec<&Activity> = self
            .favourite_activities
            .iter()
            .filter(|a| !booked(a.id) && is_bookable(a, self.day))
            .collect();
        list.sort_by(|a, b| a.full_number.cmp(&b.full_number));
        list
    }

    /// Selectable rows of the day view: the entries, then the unbooked favourites.
    pub fn day_row_count(&self) -> usize {
        self.record.as_ref().map_or(0, |r| r.entries.len()) + self.unbooked_favourites().len()
    }

    fn edit_day_row(&mut self) {
        let entries = self.record.as_ref().map_or(0, |r| r.entries.len());
        if self.sel_entry < entries {
            return self.open_edit(Some(self.sel_entry));
        }
        let favourite = self
            .unbooked_favourites()
            .get(self.sel_entry - entries)
            .map(|a| (a.id, a.full_number.clone()));
        if let Some((id, number)) = favourite {
            self.open_edit_for_activity(id, number);
        }
    }

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
            KeyCode::Char('l') | KeyCode::Char('e') | KeyCode::Enter => self.edit_day_row(),
            KeyCode::Char('n' | 'o') => self.open_edit(None),
            KeyCode::Char('d') => self.pending = "d".into(),
            KeyCode::Char('w') => self.set_view(DayView::Week),
            KeyCode::Char('a') => self.open_absence_menu(),
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
        let Some(index) = self.selected_entry() else {
            return;
        };
        let Some(mut record) = self.record.clone() else {
            return;
        };
        if index >= record.entries.len() {
            return;
        }
        record.entries.remove(index);
        self.put(record);
    }
}
