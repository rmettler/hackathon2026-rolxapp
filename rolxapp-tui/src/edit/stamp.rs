use super::validate::validate_entry;
use super::{Basis, EditForm, EditMode, Field};
use crate::app::activities::activity_label;
use crate::app::{App, Msg, StampPart, StampSession};
use crate::time::{fmt_clock, parse_clock};
use chrono::Duration;
use rolx_core::api::records::{self, Record, RecordEntry};

impl EditForm {
    /// Form that turns a stamp into entries: the first start, the last end, an activity, a comment.
    pub(crate) fn for_stamp(
        activity_id: i32,
        activity_label: String,
        from: i32,
        to: i32,
        summary: String,
    ) -> Self {
        Self {
            from: fmt_clock(from.into()),
            to: fmt_clock(to.into()),
            basis: Basis::Range,
            stamp: Some(summary),
            ..Self::blank(
                None,
                Field::Activity,
                EditMode::Search,
                activity_id,
                activity_label,
            )
        }
    }

    /// Moves to the next or previous field; a stamp has no duration field.
    pub(crate) fn step_field(&mut self, forward: bool) {
        self.field = self.field.step(forward);
        if self.stamp.is_some() && self.field == Field::Duration {
            self.field = self.field.step(forward);
        }
    }
}

fn stamp_summary(parts: &[StampPart]) -> String {
    let (Some(first), Some(last)) = (parts.first(), parts.last()) else {
        return String::new();
    };
    let at = |part: &StampPart, secs: i32| {
        format!(
            "{} {}",
            part.date.format("%a %d.%m."),
            fmt_clock(secs.into())
        )
    };
    let entries = match parts.len() {
        1 => "1 entry".to_string(),
        n => format!("{n} entries"),
    };
    format!(
        "{} - {} ({entries})",
        at(first, first.begin),
        at(last, last.end)
    )
}

impl App {
    /// Opens the form for the stamp being stamped out, once its days have loaded.
    pub(crate) fn open_stamp_form(&mut self, parts: Vec<StampPart>, records: Vec<Record>) {
        let (Some(first), Some(last)) = (parts.first(), parts.last()) else {
            return;
        };
        let Some(activity) = self.searchable().first() else {
            self.status = Some("No activities available".into());
            return;
        };
        let form = EditForm::for_stamp(
            activity.id,
            activity_label(activity),
            first.begin,
            last.end,
            stamp_summary(&parts),
        );
        self.stamp_session = Some(StampSession { parts, records });
        self.edit = Some(form);
    }

    pub(crate) fn save_stamp(&mut self) {
        match self.stamp_records() {
            Ok(records) => {
                self.edit = None;
                self.book_stamp(records);
            }
            Err(message) => self.status = Some(message),
        }
    }

    fn book_stamp(&self, records: Vec<Record>) {
        self.spawn_with_client(move |c| async move {
            records::bulk_update(&c, &records).await?;
            Ok(Msg::StampBooked(records.len()))
        });
    }

    /// The records to save: each day of the stamp with one more entry, or the reason it can't.
    pub(crate) fn stamp_records(&self) -> Result<Vec<Record>, String> {
        let (Some(form), Some(session)) = (&self.edit, &self.stamp_session) else {
            return Err("No stamp to book".into());
        };
        let (Some(from), Some(to)) = (parse_clock(&form.from), parse_clock(&form.to)) else {
            return Err("From and To must be h:mm".into());
        };

        // From moves the start on the first day, To the end on the last day.
        let mut parts = session.parts.clone();
        let count = parts.len();
        if let Some(first) = parts.first_mut() {
            first.begin = from;
        }
        if let Some(last) = parts.last_mut() {
            last.end = to;
        }
        if let Some(invalid) = parts.iter().position(|p| p.end <= p.begin) {
            let message = match (count, invalid) {
                (1, _) => "To must be after From",
                (_, 0) => "From must be before 24:00",
                _ => "To must be after 00:00",
            };
            return Err(message.into());
        }

        let number = self
            .find_activity(form.activity_id)
            .map(|a| a.full_number.clone())
            .unwrap_or_default();
        let comment = (!form.comment.trim().is_empty()).then(|| form.comment.trim().to_string());

        let mut booked = Vec::with_capacity(count);
        for part in &parts {
            let label = part.date.format("%a %d.%m.").to_string();
            if self.edit_lock.is_some_and(|lock| part.date < lock) {
                return Err(format!("{label} is before the edit lock and read-only"));
            }
            let date = part.date.format("%Y-%m-%d").to_string();
            let Some(mut record) = session.records.iter().find(|r| r.date == date).cloned() else {
                return Err(format!("{label} was not loaded"));
            };
            record.entries.push(RecordEntry {
                activity_id: form.activity_id,
                full_activity_number: number.clone(),
                duration: Duration::seconds(i64::from(part.end - part.begin)),
                begin: Some(part.begin),
                pause: None,
                comment: comment.clone(),
            });
            let index = record.entries.len() - 1;
            validate_entry(&record.entries, index).map_err(|m| format!("{label}: {m}"))?;
            booked.push(record);
        }
        Ok(booked)
    }
}
