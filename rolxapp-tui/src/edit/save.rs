use super::validate::validate_entry;
use super::{Basis, EditForm};
use crate::app::App;
use crate::time::{parse_clock, parse_duration};
use chrono::Duration;
use rolx_core::api::records::RecordEntry;

impl App {
    pub(crate) fn save_edit(&mut self) {
        let Some(form) = &self.edit else { return };
        let Some(mut record) = self.record.clone() else {
            return;
        };
        let pause = form
            .index
            .and_then(|i| record.entries.get(i))
            .and_then(|e| e.pause)
            .unwrap_or(0);
        let (duration, begin) = match form.timing(pause) {
            Ok(timing) => timing,
            Err(message) => {
                self.status = Some(message.into());
                return;
            }
        };

        let full_number = self
            .find_activity(form.activity_id)
            .map(|a| a.full_number.clone());
        let comment = (!form.comment.trim().is_empty()).then(|| form.comment.trim().to_string());

        match form.index {
            Some(i) => {
                let Some(entry) = record.entries.get_mut(i) else {
                    return;
                };
                entry.activity_id = form.activity_id;
                if let Some(number) = full_number {
                    entry.full_activity_number = number;
                }
                entry.duration = duration;
                if begin.is_some() {
                    entry.begin = begin;
                }
                entry.comment = comment;
            }
            None => record.entries.push(RecordEntry {
                activity_id: form.activity_id,
                full_activity_number: full_number.unwrap_or_default(),
                duration,
                begin,
                pause: None,
                comment,
            }),
        }
        let edited = form.index.unwrap_or(record.entries.len() - 1);
        if let Err(message) = validate_entry(&record.entries, edited) {
            self.status = Some(message);
            return;
        }
        self.edit = None;
        self.put(record);
    }
}

impl EditForm {
    /// The duration and new start time (if any) the form asks for.
    /// `pause` is the existing pause of the entry, which stays inside From..To.
    fn timing(&self, pause: i32) -> Result<(Duration, Option<i32>), &'static str> {
        match self.basis {
            Basis::Duration => parse_duration(&self.duration)
                .map(|d| (d, None))
                .ok_or("Duration must be h:mm or decimal hours"),
            Basis::Range => match (parse_clock(&self.from), parse_clock(&self.to)) {
                (Some(from), Some(to)) if to - from - pause > 0 => {
                    Ok((Duration::seconds((to - from - pause).into()), Some(from)))
                }
                (Some(_), Some(_)) => Err("To must be after From"),
                _ => Err("From and To must be h:mm"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{EditMode, Field};

    fn form(basis: Basis, from: &str, to: &str, duration: &str) -> EditForm {
        EditForm {
            from: from.into(),
            to: to.into(),
            duration: duration.into(),
            basis,
            ..EditForm::for_new(1, String::new())
        }
    }

    #[test]
    fn duration_basis_keeps_the_start_time() {
        let f = form(Basis::Duration, "09:00", "10:00", "1:30");
        assert_eq!(f.timing(0), Ok((Duration::minutes(90), None)));
        assert!(f.mode == EditMode::Search && f.field == Field::Activity);
    }

    #[test]
    fn range_basis_subtracts_the_pause() {
        let f = form(Basis::Range, "9:00", "10:30", "");
        assert_eq!(f.timing(0), Ok((Duration::minutes(90), Some(9 * 3600))));
        assert_eq!(f.timing(1800), Ok((Duration::minutes(60), Some(9 * 3600))));
    }

    #[test]
    fn range_basis_rejects_bad_input() {
        assert!(form(Basis::Range, "10:00", "9:00", "").timing(0).is_err());
        assert!(form(Basis::Range, "9:00", "", "").timing(0).is_err());
        assert!(form(Basis::Duration, "", "", "abc").timing(0).is_err());
    }
}
