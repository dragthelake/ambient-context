use chrono::{DateTime, Local, NaiveDate, NaiveTime, TimeZone};

/// A first run after months of capture would otherwise summarise every day
/// at once, on the user's own subscription. Older days stay available to
/// summarise on demand from the Day view.
pub const MAX_BACKFILL_DAYS: usize = 7;

/// The days that should be summarised now. Empty when no schedule is set,
/// when the scheduled moment has not passed since the last run, or when
/// every finished day already has a summary.
pub fn due(
    now: DateTime<Local>,
    schedule_hhmm: Option<&str>,
    last_run: Option<DateTime<Local>>,
    captured: &[NaiveDate],
    summarised: &[NaiveDate],
) -> Vec<NaiveDate> {
    let Some(raw) = schedule_hhmm else {
        return Vec::new();
    };
    let Ok(time) = NaiveTime::parse_from_str(raw, "%H:%M") else {
        return Vec::new();
    };

    let today = now.date_naive();
    // The most recent occurrence of the scheduled time at or before now.
    let occurrence_date = if now.time() >= time {
        today
    } else {
        today.pred_opt().unwrap_or(today)
    };
    let Some(occurrence) = Local
        .from_local_datetime(&occurrence_date.and_time(time))
        .single()
    else {
        // Ambiguous or skipped local time across a daylight-saving change.
        // Treat it as not due rather than guessing; the next tick resolves.
        return Vec::new();
    };

    let overdue = match last_run {
        None => true,
        Some(last) => last < occurrence,
    };
    if !overdue {
        return Vec::new();
    }

    let mut pending: Vec<NaiveDate> = captured
        .iter()
        .copied()
        .filter(|date| *date < today && !summarised.contains(date))
        .collect();
    pending.sort();
    if pending.len() > MAX_BACKFILL_DAYS {
        pending = pending.split_off(pending.len() - MAX_BACKFILL_DAYS);
    }
    pending
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, NaiveDate, TimeZone};

    fn at(y: i32, m: u32, d: u32, hh: u32, mm: u32) -> chrono::DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, hh, mm, 0).unwrap()
    }

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn nothing_is_due_without_a_schedule() {
        let captured = vec![day(2026, 8, 28)];
        assert!(due(at(2026, 8, 29, 9, 0), None, None, &captured, &[]).is_empty());
    }

    #[test]
    fn nothing_is_due_before_the_scheduled_time_when_it_already_ran_yesterday() {
        let captured = vec![day(2026, 8, 28)];
        let last = Some(at(2026, 8, 28, 6, 0));
        assert!(due(at(2026, 8, 29, 5, 0), Some("06:00"), last, &captured, &[]).is_empty());
    }

    #[test]
    fn yesterday_is_due_once_the_scheduled_time_has_passed() {
        let captured = vec![day(2026, 8, 28)];
        let last = Some(at(2026, 8, 28, 6, 0));
        let out = due(at(2026, 8, 29, 6, 1), Some("06:00"), last, &captured, &[]);
        assert_eq!(out, vec![day(2026, 8, 28)]);
    }

    #[test]
    fn today_is_never_summarised_because_the_day_is_not_finished() {
        let captured = vec![day(2026, 8, 28), day(2026, 8, 29)];
        let out = due(at(2026, 8, 29, 7, 0), Some("06:00"), None, &captured, &[]);
        assert_eq!(out, vec![day(2026, 8, 28)]);
    }

    #[test]
    fn days_that_already_have_summaries_are_skipped() {
        let captured = vec![day(2026, 8, 27), day(2026, 8, 28)];
        let summarised = vec![day(2026, 8, 27)];
        let out = due(
            at(2026, 8, 29, 7, 0),
            Some("06:00"),
            None,
            &captured,
            &summarised,
        );
        assert_eq!(out, vec![day(2026, 8, 28)]);
    }

    #[test]
    fn a_first_run_backfills_but_not_without_limit() {
        // Enabling summaries after months of capture must not spend months
        // of the user's tokens in one morning.
        let captured: Vec<NaiveDate> = (1..=28).map(|d| day(2026, 8, d)).collect();
        let out = due(at(2026, 8, 29, 7, 0), Some("06:00"), None, &captured, &[]);
        assert_eq!(out.len(), MAX_BACKFILL_DAYS);
        assert_eq!(*out.last().unwrap(), day(2026, 8, 28));
    }

    #[test]
    fn due_days_come_back_oldest_first() {
        let captured = vec![day(2026, 8, 26), day(2026, 8, 27), day(2026, 8, 28)];
        let out = due(at(2026, 8, 29, 7, 0), Some("06:00"), None, &captured, &[]);
        assert_eq!(
            out,
            vec![day(2026, 8, 26), day(2026, 8, 27), day(2026, 8, 28)]
        );
    }

    #[test]
    fn a_missed_night_catches_up_rather_than_being_skipped() {
        // Machine asleep at 06:00, opened at 14:00 the next day.
        let captured = vec![day(2026, 8, 27), day(2026, 8, 28)];
        let last = Some(at(2026, 8, 27, 6, 0));
        let out = due(at(2026, 8, 29, 14, 0), Some("06:00"), last, &captured, &[]);
        assert_eq!(out, vec![day(2026, 8, 27), day(2026, 8, 28)]);
    }

    #[test]
    fn a_malformed_schedule_string_means_no_schedule_rather_than_a_panic() {
        let captured = vec![day(2026, 8, 28)];
        assert!(
            due(at(2026, 8, 29, 9, 0), Some("nonsense"), None, &captured, &[]).is_empty()
        );
    }
}
