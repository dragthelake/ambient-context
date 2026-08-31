use crate::{summarise, writer};
use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::Path;

/// One day as the window sees it. `has_capture` and `has_summary` are the
/// two marks the calendar draws; `bytes` is the day file's size, which is
/// the only volume signal the header has; `title` is the summary's own
/// one-line name for the day.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DayEntry {
    pub date: NaiveDate,
    pub has_capture: bool,
    pub has_summary: bool,
    pub bytes: u64,
    pub title: Option<String>,
}

fn entry(folder: &Path, date: NaiveDate) -> DayEntry {
    let day_path = writer::file_path(folder, date);
    let summary = std::fs::read_to_string(summarise::summary_path(folder, date)).ok();
    DayEntry {
        date,
        has_capture: day_path.is_file(),
        has_summary: summary.is_some(),
        bytes: std::fs::metadata(&day_path).map(|m| m.len()).unwrap_or(0),
        title: summary.as_deref().and_then(summarise::title_of),
    }
}

/// Every date with a day file or a summary, deduplicated and sorted.
fn known_dates(folder: &Path) -> BTreeSet<NaiveDate> {
    let mut dates: BTreeSet<NaiveDate> = summarise::list_captured(folder).into_iter().collect();
    dates.extend(summarise::list_summarised(folder));
    dates
}

/// Newest first: the window opens on today and the recent past is what the
/// arrows walk through.
pub fn list_days(folder: &Path) -> Vec<DayEntry> {
    known_dates(folder)
        .into_iter()
        .rev()
        .map(|date| entry(folder, date))
        .collect()
}

/// Oldest first, which is the order a calendar grid fills.
pub fn days_in_month(folder: &Path, year: i32, month: u32) -> Vec<DayEntry> {
    known_dates(folder)
        .into_iter()
        .filter(|date| date.year() == year && date.month() == month)
        .map(|date| entry(folder, date))
        .collect()
}

pub fn read_day(folder: &Path, date: NaiveDate) -> Option<String> {
    std::fs::read_to_string(writer::file_path(folder, date)).ok()
}

pub fn read_summary(folder: &Path, date: NaiveDate) -> Option<String> {
    std::fs::read_to_string(summarise::summary_path(folder, date)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    /// Two captured days in August, one of them summarised, one summary in
    /// July whose day file has been deleted, and three files that are not
    /// days at all.
    fn folder() -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("2026-08-27.md"), "twelve bytes").unwrap();
        std::fs::write(dir.path().join("2026-08-28.md"), "x").unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "x").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "x").unwrap();
        std::fs::create_dir_all(dir.path().join("Ledger")).unwrap();
        std::fs::write(dir.path().join("Ledger").join("2026-08-28.md"), "x").unwrap();
        std::fs::create_dir_all(dir.path().join("Summaries")).unwrap();
        std::fs::write(
            dir.path().join("Summaries").join("2026-08-27.md"),
            "---\ndate: 2026-08-27\n---\n\n# A day of plumbing\n\nprose",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("Summaries").join("2026-07-30.md"),
            "---\ndate: 2026-07-30\n---\n\n# An older day\n\nprose",
        )
        .unwrap();
        dir
    }

    #[test]
    fn days_come_back_newest_first() {
        let dir = folder();
        let dates: Vec<NaiveDate> = list_days(dir.path()).iter().map(|d| d.date).collect();
        assert_eq!(
            dates,
            vec![date(2026, 8, 28), date(2026, 8, 27), date(2026, 7, 30)]
        );
    }

    #[test]
    fn a_day_carries_its_marks_its_size_and_its_title() {
        let dir = folder();
        let days = list_days(dir.path());
        let twenty_seventh = days.iter().find(|d| d.date == date(2026, 8, 27)).unwrap();
        assert!(twenty_seventh.has_capture);
        assert!(twenty_seventh.has_summary);
        assert_eq!(twenty_seventh.bytes, 12);
        assert_eq!(twenty_seventh.title, Some("A day of plumbing".to_string()));
    }

    #[test]
    fn a_captured_day_with_no_summary_says_so() {
        let dir = folder();
        let days = list_days(dir.path());
        let twenty_eighth = days.iter().find(|d| d.date == date(2026, 8, 28)).unwrap();
        assert!(twenty_eighth.has_capture);
        assert!(!twenty_eighth.has_summary);
        assert_eq!(twenty_eighth.title, None);
    }

    #[test]
    fn a_summary_whose_day_file_is_gone_still_appears() {
        // Deletion is Finder, and a user who deletes a day file must not
        // lose the summary from the calendar.
        let dir = folder();
        let days = list_days(dir.path());
        let july = days.iter().find(|d| d.date == date(2026, 7, 30)).unwrap();
        assert!(!july.has_capture);
        assert!(july.has_summary);
        assert_eq!(july.bytes, 0);
    }

    #[test]
    fn a_month_holds_only_its_own_days_oldest_first() {
        let dir = folder();
        let dates: Vec<NaiveDate> = days_in_month(dir.path(), 2026, 8)
            .iter()
            .map(|d| d.date)
            .collect();
        assert_eq!(dates, vec![date(2026, 8, 27), date(2026, 8, 28)]);
    }

    #[test]
    fn a_month_with_nothing_in_it_is_empty_rather_than_an_error() {
        let dir = folder();
        assert!(days_in_month(dir.path(), 2026, 6).is_empty());
    }

    #[test]
    fn reading_returns_the_day_and_the_summary_and_none_when_absent() {
        let dir = folder();
        assert_eq!(
            read_day(dir.path(), date(2026, 8, 27)).unwrap(),
            "twelve bytes"
        );
        assert!(read_summary(dir.path(), date(2026, 8, 27))
            .unwrap()
            .contains("A day of plumbing"));
        assert_eq!(read_day(dir.path(), date(2026, 1, 1)), None);
        assert_eq!(read_summary(dir.path(), date(2026, 8, 28)), None);
    }

    #[test]
    fn a_folder_that_does_not_exist_lists_nothing_rather_than_panicking() {
        assert!(list_days(std::path::Path::new("/nope/not/here")).is_empty());
    }
}
