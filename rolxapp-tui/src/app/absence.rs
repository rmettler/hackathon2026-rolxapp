use super::App;
use chrono::Duration;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use rolx_core::api::records::Record;

/// Paid leave types the server accepts without a written reason.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Absence {
    Sickness,
    Vacation,
    Military,
}

/// The server's `DayType.Weekend`.
const WEEKEND: i32 = 1;

impl Absence {
    pub const ALL: [Absence; 3] = [Absence::Sickness, Absence::Vacation, Absence::Military];

    pub fn label(self) -> &'static str {
        match self {
            Absence::Sickness => "Krank",
            Absence::Vacation => "Ferien",
            Absence::Military => "Militär",
        }
    }

    /// Fits the six-character week grid columns.
    pub fn short(self) -> &'static str {
        match self {
            Absence::Sickness => "Krank",
            Absence::Vacation => "Ferien",
            Absence::Military => "Milit.",
        }
    }

    /// The server's `PaidLeaveType` number.
    pub fn code(self) -> i32 {
        match self {
            Absence::Vacation => 0,
            Absence::Sickness => 1,
            Absence::Military => 2,
        }
    }

    pub fn from_code(code: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.code() == code)
    }
}

/// The absence booked on the record's day, if it is one this app can set.
pub fn absence_of(record: &Record) -> Option<Absence> {
    record.paid_leave_type.and_then(Absence::from_code)
}

/// Time the server credits as leave: the day's target minus the booked time.
pub fn leave_time(record: &Record) -> Option<Duration> {
    record.paid_leave_type?;
    let booked: Duration = record.entries.iter().map(|e| e.duration).sum();
    Some((Duration::seconds(record.nominal_work_time) - booked).max(Duration::zero()))
}

impl App {
    pub(crate) fn open_absence_menu(&mut self) {
        if self.record.as_ref().is_some_and(|r| r.day_type == WEEKEND) {
            self.status = Some("Weekends can't take an absence".into());
            return;
        }
        if !self.can_edit() {
            return;
        }
        let current = self.record.as_ref().and_then(absence_of);
        let selected = Absence::ALL.iter().position(|a| Some(*a) == current);
        self.absence_menu = Some(selected.unwrap_or(0));
    }

    pub(crate) fn on_absence_key(&mut self, key: KeyEvent) {
        let Some(selected) = self.absence_menu else {
            return;
        };
        let last = Absence::ALL.len() - 1;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.absence_menu = None,
            KeyCode::Up | KeyCode::Char('k') => {
                self.absence_menu = Some(selected.saturating_sub(1));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.absence_menu = Some((selected + 1).min(last));
            }
            KeyCode::Char(c @ '1'..='3') => {
                self.book_absence(Absence::ALL[c as usize - '1' as usize]);
            }
            KeyCode::Enter => self.book_absence(Absence::ALL[selected]),
            _ => {}
        }
    }

    fn book_absence(&mut self, absence: Absence) {
        self.absence_menu = None;
        match self.record_with_absence(absence) {
            Ok(record) => self.put(record),
            Err(message) => self.status = Some(message),
        }
    }

    /// The selected day's record with `absence` set, or cleared when it already is that one.
    pub(crate) fn record_with_absence(&self, absence: Absence) -> Result<Record, String> {
        let Some(record) = &self.record else {
            return Err("Nothing loaded to edit".into());
        };
        if self.is_locked() {
            return Err("Day is before the edit lock and read-only".into());
        }
        if record.day_type == WEEKEND {
            return Err("Weekends can't take an absence".into());
        }

        let mut record = record.clone();
        if record.paid_leave_type == Some(absence.code()) {
            record.paid_leave_type = None;
        } else {
            let booked: Duration = record.entries.iter().map(|e| e.duration).sum();
            if booked > Duration::seconds(record.nominal_work_time) {
                return Err("More time is booked than the day's target, no absence fits".into());
            }
            record.paid_leave_type = Some(absence.code());
        }
        record.paid_leave_reason = None;
        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::test_support::main_app;

    fn app_with(paid_leave_type: Option<i32>) -> App {
        let mut app = main_app();
        app.record.as_mut().unwrap().paid_leave_type = paid_leave_type;
        app
    }

    #[test]
    fn maps_to_the_servers_numbers() {
        assert_eq!(Absence::Vacation.code(), 0);
        assert_eq!(Absence::Sickness.code(), 1);
        assert_eq!(Absence::Military.code(), 2);
        assert_eq!(Absence::from_code(1), Some(Absence::Sickness));
        assert_eq!(Absence::from_code(3), None);
    }

    #[test]
    fn sets_the_type_and_keeps_the_entries() {
        let app = app_with(None);
        let record = app.record_with_absence(Absence::Vacation).unwrap();

        assert_eq!(record.paid_leave_type, Some(0));
        assert_eq!(record.paid_leave_reason, None);
        assert_eq!(record.entries.len(), 1);
    }

    #[test]
    fn another_type_replaces_the_current_one() {
        let app = app_with(Some(1));
        let record = app.record_with_absence(Absence::Military).unwrap();

        assert_eq!(record.paid_leave_type, Some(2));
    }

    #[test]
    fn the_same_type_again_clears_it() {
        let app = app_with(Some(1));
        let record = app.record_with_absence(Absence::Sickness).unwrap();

        assert_eq!(record.paid_leave_type, None);
    }

    #[test]
    fn locked_days_are_refused() {
        let mut app = app_with(None);
        app.edit_lock = NaiveDate::from_ymd_opt(2026, 11, 1);

        let error = app.record_with_absence(Absence::Sickness).unwrap_err();
        assert!(error.contains("read-only"));
    }

    #[test]
    fn overtime_days_are_refused_but_can_be_cleared() {
        let mut app = app_with(None);
        app.record.as_mut().unwrap().entries[0].duration = Duration::hours(9);
        assert!(app.record_with_absence(Absence::Sickness).is_err());

        app.record.as_mut().unwrap().paid_leave_type = Some(1);
        let record = app.record_with_absence(Absence::Sickness).unwrap();
        assert_eq!(record.paid_leave_type, None);
    }

    #[test]
    fn weekends_are_ignored() {
        let mut app = app_with(None);
        app.record.as_mut().unwrap().day_type = 1;

        assert!(app.record_with_absence(Absence::Vacation).is_err());
        app.open_absence_menu();
        assert_eq!(app.absence_menu, None);
        assert!(app.status.as_deref().unwrap_or("").contains("Weekend"));
    }

    #[test]
    fn public_holidays_can_take_an_absence() {
        let mut app = app_with(None);
        let record = app.record.as_mut().unwrap();
        record.day_type = 2;
        record.nominal_work_time = 0;
        record.entries.clear();

        let record = app.record_with_absence(Absence::Vacation).unwrap();
        assert_eq!(record.paid_leave_type, Some(0));
    }

    #[test]
    fn leave_time_is_the_target_minus_booked() {
        let app = app_with(Some(1));
        let record = app.record.as_ref().unwrap();
        // 8:00 target, 1:30 booked.
        assert_eq!(leave_time(record), Some(Duration::minutes(390)));

        assert_eq!(leave_time(app_with(None).record.as_ref().unwrap()), None);
    }
}
