use crate::time::{entry_end, fmt_clock};
use chrono::Duration;
use rolx_core::api::records::RecordEntry;

/// Checks that the entry at `edited` fits the day and does not overlap other timed entries.
pub(crate) fn validate_entry(entries: &[RecordEntry], edited: usize) -> Result<(), String> {
    let entry = &entries[edited];
    if entry.duration <= Duration::zero() {
        return Err("Duration must be greater than 0".into());
    }
    let Some(begin) = entry.begin else {
        return Ok(());
    };
    let (start, end) = (i64::from(begin), entry_end(begin, entry));
    if end > 24 * 3600 {
        return Err("Entry must end by 24:00".into());
    }
    for (i, other) in entries.iter().enumerate() {
        let Some(other_begin) = other.begin else {
            continue;
        };
        let (other_start, other_end) = (i64::from(other_begin), entry_end(other_begin, other));
        if i != edited && other_start < other_end && start < other_end && other_start < end {
            return Err(format!(
                "Overlaps {} {}-{}",
                other.full_activity_number,
                fmt_clock(other_start),
                fmt_clock(other_end)
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timed(begin: i32, minutes: i64) -> RecordEntry {
        RecordEntry {
            activity_id: 1,
            full_activity_number: "P1-A1".into(),
            duration: Duration::minutes(minutes),
            begin: Some(begin * 3600),
            pause: None,
            comment: None,
        }
    }

    #[test]
    fn rejects_overlapping_entries() {
        let entries = [timed(9, 120), timed(10, 60)];
        assert!(
            validate_entry(&entries, 1)
                .unwrap_err()
                .contains("Overlaps")
        );
        assert!(validate_entry(&entries, 0).is_err());
    }

    #[test]
    fn accepts_adjacent_and_untimed_entries() {
        let mut untimed = timed(10, 60);
        untimed.begin = None;
        let entries = [timed(9, 60), timed(10, 60), untimed];
        assert!(validate_entry(&entries, 1).is_ok());
        assert!(validate_entry(&entries, 2).is_ok());
    }

    #[test]
    fn rejects_zero_duration_and_past_midnight() {
        assert!(validate_entry(&[timed(9, 0)], 0).is_err());
        assert!(validate_entry(&[timed(23, 120)], 0).is_err());
    }
}
