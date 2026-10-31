use super::{App, DayView, Screen, Tab};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    pub(crate) fn on_key(&mut self, key: KeyEvent) {
        self.status = None;
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        match &self.screen {
            Screen::Connecting => {
                if key.code == KeyCode::Char('q') {
                    self.quit = true;
                }
            }
            Screen::Login { busy, .. } => match key.code {
                KeyCode::Char('q') => self.quit = true,
                KeyCode::Char('l') | KeyCode::Enter if !busy => self.log_in(),
                _ => {}
            },
            Screen::Main if self.discard_prompt => self.on_discard_key(key),
            Screen::Main if self.edit.is_some() => self.on_edit_key(key),
            Screen::Main if self.absence_menu.is_some() => self.on_absence_key(key),
            Screen::Main if self.act_searching => self.on_activity_filter_key(key),
            Screen::Main => self.on_main_key(key),
        }
    }

    pub(crate) fn on_main_key(&mut self, key: KeyEvent) {
        let pending = std::mem::take(&mut self.pending);
        if !pending.is_empty() {
            self.on_pending(&pending, key);
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            let (half, full) = (self.half_page(), self.full_page());
            match key.code {
                KeyCode::Char('d') => self.scroll(half),
                KeyCode::Char('u') => self.scroll(-half),
                KeyCode::Char('f') => self.scroll(full),
                KeyCode::Char('b') => self.scroll(-full),
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('g') => self.pending = "g".into(),
            KeyCode::Char('G') => self.goto_edge(true),
            KeyCode::PageDown => self.scroll(self.full_page()),
            KeyCode::PageUp => self.scroll(-self.full_page()),
            KeyCode::Tab => self.set_tab((self.tab.index() + 1) % Tab::ALL.len()),
            KeyCode::BackTab => {
                self.set_tab((self.tab.index() + Tab::ALL.len() - 1) % Tab::ALL.len());
            }
            KeyCode::Char(c @ '1'..='5') => self.set_tab(c as usize - '1' as usize),
            KeyCode::Char('r') => self.refresh(),
            KeyCode::Char('s') => self.toggle_stamp(),
            KeyCode::Char('S') => self.begin_discard(),
            _ => match self.tab {
                Tab::Day => match self.day_view {
                    DayView::Day => self.on_day_key(key),
                    DayView::Week => self.on_week_key(key),
                },
                Tab::Activities => self.on_activities_key(key),
                Tab::Year => self.on_year_key(key),
                Tab::Report => self.on_report_key(key),
                Tab::Charts => self.on_charts_key(key),
            },
        }
    }

    /// Second key of a multi-key command; anything unknown cancels it.
    pub(crate) fn on_pending(&mut self, pending: &str, key: KeyEvent) {
        let next = (self.tab.index() + 1) % Tab::ALL.len();
        let previous = (self.tab.index() + Tab::ALL.len() - 1) % Tab::ALL.len();
        match (pending, key.code) {
            ("g", KeyCode::Char('g')) => self.goto_edge(false),
            ("g", KeyCode::Char('t')) => self.set_tab(next),
            ("g", KeyCode::Char('T')) => self.set_tab(previous),
            ("d", KeyCode::Char('d')) if self.tab == Tab::Day => self.delete_entry(),
            _ => {}
        }
    }

    pub(crate) fn set_tab(&mut self, index: usize) {
        self.tab = Tab::ALL[index];
        if self.tab == Tab::Charts && self.chart_records.is_none() {
            self.load_charts();
        }
    }

    pub(crate) fn refresh(&self) {
        self.load_header();
        match self.tab {
            Tab::Day => self.load_view(),
            Tab::Activities => self.load_activities(),
            Tab::Year => self.load_year(),
            Tab::Report => self.load_report(),
            Tab::Charts => self.load_charts(),
        }
    }
}
