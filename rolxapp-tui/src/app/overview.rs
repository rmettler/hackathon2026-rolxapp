use super::App;
use chrono::Datelike;
use chrono::Duration;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

impl App {
    pub(crate) fn on_year_key(&mut self, key: KeyEvent) {
        let delta = match key.code {
            KeyCode::Left | KeyCode::Char('h') => -1,
            KeyCode::Right | KeyCode::Char('l') => 1,
            KeyCode::Up | KeyCode::Char('k') => return self.scroll(-1),
            KeyCode::Down | KeyCode::Char('j') => return self.scroll(1),
            _ => return,
        };
        self.year += delta;
        self.year_info = None;
        self.year_scroll = 0;
        self.load_year();
    }

    pub(crate) fn on_report_key(&mut self, key: KeyEvent) {
        let month = match key.code {
            KeyCode::Left | KeyCode::Char('h') => self.month - Duration::days(1),
            KeyCode::Right | KeyCode::Char('l') => self.month + Duration::days(32),
            KeyCode::Up | KeyCode::Char('k') => return self.scroll(-1),
            KeyCode::Down | KeyCode::Char('j') => return self.scroll(1),
            _ => return,
        };
        self.month = month.with_day(1).unwrap_or(month);
        self.report = None;
        self.report_scroll = 0;
        self.load_report();
    }
}
