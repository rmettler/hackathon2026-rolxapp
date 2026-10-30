use super::App;
use super::activities::is_bookable;
use super::week_grid::WeekGrid;
use crate::time::week_start;
use chrono::{Datelike, Duration, Local, NaiveDate};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

/// What the Day tab shows.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DayView {
    Day,
    Week,
}

impl App {
    pub fn week_grid(&self) -> WeekGrid {
        WeekGrid::build(week_start(self.day), &self.week, &self.favourite_activities)
    }

    /// Selected weekday column, Monday = 0.
    pub fn sel_col(&self) -> usize {
        self.day.weekday().num_days_from_monday() as usize
    }

    pub(crate) fn load_view(&self) {
        match self.day_view {
            DayView::Day => self.load_day(),
            DayView::Week => self.load_week(),
        }
    }

    pub(crate) fn set_view(&mut self, view: DayView) {
        self.day_view = view;
        self.sel_entry = 0;
        self.sel_row = 0;
        match view {
            DayView::Day => self.load_day(),
            DayView::Week => {
                self.week.clear();
                self.load_week();
            }
        }
    }

    /// Points `record` at the selected weekday's record of the loaded week.
    pub(crate) fn sync_week_record(&mut self) {
        let date = self.day.format("%Y-%m-%d").to_string();
        self.record = self.week.iter().find(|r| r.date == date).cloned();
        self.sel_entry = 0;
    }

    fn move_week_day(&mut self, day: NaiveDate) {
        let same_week = week_start(day) == week_start(self.day);
        self.day = day;
        if same_week {
            self.sync_week_record();
        } else {
            self.week.clear();
            self.record = None;
            self.sel_row = 0;
            self.load_week();
        }
    }

    pub(crate) fn on_week_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => self.move_week_day(self.day - Duration::days(1)),
            KeyCode::Right | KeyCode::Char('l') => self.move_week_day(self.day + Duration::days(1)),
            KeyCode::Char('[') => self.move_week_day(self.day - Duration::days(7)),
            KeyCode::Char(']') => self.move_week_day(self.day + Duration::days(7)),
            KeyCode::Char('t') => self.move_week_day(Local::now().date_naive()),
            KeyCode::Up | KeyCode::Char('k') => self.scroll(-1),
            KeyCode::Down | KeyCode::Char('j') => self.scroll(1),
            KeyCode::Char('w') => self.set_view(DayView::Day),
            KeyCode::Char('a') => self.open_absence_menu(),
            KeyCode::Char('e' | 'i') | KeyCode::Enter => self.edit_cell(),
            KeyCode::Char('n' | 'o') => self.open_edit(None),
            KeyCode::Char('d') => self.pending = "d".into(),
            _ => {}
        }
    }

    /// Indices into the selected day's entries that make up the selected cell.
    fn cell_entries(&self) -> Vec<usize> {
        let grid = self.week_grid();
        let (Some(row), Some(record)) = (grid.rows.get(self.sel_row), &self.record) else {
            return Vec::new();
        };
        record
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.activity_id == row.activity_id)
            .map(|(i, _)| i)
            .collect()
    }

    fn edit_cell(&mut self) {
        let grid = self.week_grid();
        let Some(row) = grid.rows.get(self.sel_row) else {
            return self.open_edit(None);
        };
        let (activity_id, number) = (row.activity_id, row.number.clone());
        match self.cell_entries().as_slice() {
            [] => {
                let bookable = self
                    .find_activity(activity_id)
                    .is_none_or(|a| is_bookable(a, self.day));
                if bookable {
                    self.open_edit_for_activity(activity_id, number);
                } else {
                    self.status = Some("This activity can't be booked on that day".into());
                }
            }
            [index] => self.open_edit(Some(*index)),
            [first, ..] => {
                self.sel_entry = *first;
                self.day_view = DayView::Day;
                self.status = Some("Several entries in this cell, edit them here".into());
                self.load_day();
            }
        }
    }

    /// Index of the entry the edit and delete commands act on, if there is exactly one.
    pub(crate) fn selected_entry(&mut self) -> Option<usize> {
        match self.day_view {
            DayView::Day => Some(self.sel_entry),
            DayView::Week => match self.cell_entries().as_slice() {
                [index] => Some(*index),
                [] => None,
                _ => {
                    self.status =
                        Some("Several entries in this cell, delete them in the day view".into());
                    None
                }
            },
        }
    }
}
