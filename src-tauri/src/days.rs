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

/// One block as the Raw view shows it. Times stay as the `HH:MM` strings
/// the writer wrote, because that is what is displayed and reparsing them
/// into a `DateTime` would need the day's date and gain nothing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RawBlock {
    pub start: String,
    pub end: String,
    pub app: String,
    pub title: Option<String>,
    pub file: Option<String>,
    pub url: Option<String>,
    pub lines: Vec<String>,
}

/// Splits `## HH:MM–HH:MM · App · Title` on the separators the writer uses:
/// an en dash between the times and a middle dot between the fields. The
/// title is whatever is left after the app, separators and all, because a
/// Slack window title contains middle dots of its own.
fn parse_heading(line: &str) -> Option<(String, String, String, Option<String>)> {
    let rest = line.strip_prefix("## ")?;
    let mut fields = rest.splitn(3, '\u{00b7}');
    let times = fields.next()?.trim();
    let app = fields.next()?.trim();
    let title = fields.next().map(str::trim).filter(|t| !t.is_empty());
    let (start, end) = times.split_once('\u{2013}')?;
    let start = start.trim();
    let end = end.trim();
    if start.len() != 5 || end.len() != 5 || app.is_empty() {
        return None;
    }
    Some((
        start.to_string(),
        end.to_string(),
        app.to_string(),
        title.map(str::to_string),
    ))
}

pub fn parse_blocks(day_text: &str) -> Vec<RawBlock> {
    let mut blocks: Vec<RawBlock> = Vec::new();
    for line in day_text.lines() {
        if line.starts_with("## ") {
            if let Some((start, end, app, title)) = parse_heading(line) {
                blocks.push(RawBlock {
                    start,
                    end,
                    app,
                    title,
                    file: None,
                    url: None,
                    lines: Vec::new(),
                });
            }
            continue;
        }
        let Some(block) = blocks.last_mut() else {
            continue; // frontmatter, or anything before the first heading
        };
        if let Some(path) = line.strip_prefix("file: ") {
            block.file = Some(path.to_string());
        } else if let Some(url) = line.strip_prefix("url: ") {
            block.url = Some(url.to_string());
        } else if !line.trim().is_empty() {
            block.lines.push(line.to_string());
        }
    }
    blocks
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

    const DAY: &str = "---\ndate: 2026-08-25\ncaptured_by: Ambient Context 0.3.0\n---\n\n## 09:14\u{2013}09:41 \u{00b7} Linear \u{00b7} YN-102 Proposal protocol\n\nfile: /Users/x/report.pdf\nurl: https://linear.app/empty/issue/YN-102\n\nread the issue\nwrote a comment\n\n## 09:41\u{2013}10:02 \u{00b7} Safari\n\nsome page text\n\n## 10:02\u{2013}10:20 \u{00b7} Slack \u{00b7} #empty-build \u{00b7} thread\n";

    #[test]
    fn parses_every_block_in_a_day() {
        let blocks = parse_blocks(DAY);
        assert_eq!(blocks.len(), 3);
    }

    #[test]
    fn parses_the_heading_into_times_app_and_title() {
        let block = &parse_blocks(DAY)[0];
        assert_eq!(block.start, "09:14");
        assert_eq!(block.end, "09:41");
        assert_eq!(block.app, "Linear");
        assert_eq!(block.title.as_deref(), Some("YN-102 Proposal protocol"));
    }

    #[test]
    fn a_block_with_no_title_has_none() {
        let block = &parse_blocks(DAY)[1];
        assert_eq!(block.app, "Safari");
        assert_eq!(block.title, None);
    }

    #[test]
    fn a_title_containing_the_separator_is_kept_whole() {
        let block = &parse_blocks(DAY)[2];
        assert_eq!(block.app, "Slack");
        assert_eq!(block.title.as_deref(), Some("#empty-build \u{00b7} thread"));
    }

    #[test]
    fn references_are_lifted_out_of_the_body() {
        let block = &parse_blocks(DAY)[0];
        assert_eq!(block.file.as_deref(), Some("/Users/x/report.pdf"));
        assert_eq!(
            block.url.as_deref(),
            Some("https://linear.app/empty/issue/YN-102")
        );
        assert_eq!(
            block.lines,
            vec!["read the issue".to_string(), "wrote a comment".to_string()]
        );
    }

    #[test]
    fn a_headings_only_block_has_no_lines() {
        let block = &parse_blocks(DAY)[2];
        assert!(block.lines.is_empty());
    }

    #[test]
    fn frontmatter_never_becomes_a_block() {
        assert!(parse_blocks(DAY).iter().all(|b| !b.app.contains("captured_by")));
    }

    #[test]
    fn an_empty_or_malformed_file_yields_no_blocks() {
        assert!(parse_blocks("").is_empty());
        assert!(parse_blocks("## not a heading we wrote\n\nbody\n").is_empty());
        assert!(parse_blocks("## 09:14 \u{00b7} Linear\n").is_empty());
    }
}
