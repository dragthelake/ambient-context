use crate::segment::Block;
use chrono::{Duration, NaiveDate};
use std::path::Path;

/// A block has to carry at least this many lines before the overlap rule
/// will look at it. Two lines in common with an old summary is a
/// coincidence; three is a re-reading.
const MIN_LINES: usize = 3;

/// How far back the overlap rule reads. A summary older than a week is
/// rarely reopened, and each extra day is another file read at block close.
const LOOK_BACK_DAYS: i64 = 7;

/// The date embedded in a path to a record of an earlier day: the
/// `Summaries/YYYY-MM-DD.md` the app writes, or a `KB/YYYY-MM-DD/` file
/// underneath it.
fn date_in_reference(reference: &str) -> Option<NaiveDate> {
    for (marker, suffix) in [("/Summaries/", ".md"), ("/KB/", "/")] {
        let Some(index) = reference.rfind(marker) else {
            continue;
        };
        let rest = &reference[index + marker.len()..];
        let (Some(head), Some(tail)) = (rest.get(..10), rest.get(10..)) else {
            continue;
        };
        if !tail.starts_with(suffix) {
            continue;
        }
        if let Ok(date) = NaiveDate::parse_from_str(head, "%Y-%m-%d") {
            return Some(date);
        }
    }
    None
}

/// The block's lines that appear verbatim as a line of `summary`.
fn overlap(lines: &[&str], summary: &str) -> usize {
    let summary_lines: Vec<&str> = summary.lines().map(str::trim).collect();
    lines
        .iter()
        .filter(|line| summary_lines.contains(line))
        .count()
}

/// Whether `summary` accounts for more than half of the block's lines, and
/// at least three of them. Half is the threshold because an editor window
/// showing an old summary also shows its own chrome: a tab bar, a path, a
/// line number.
fn is_replay_of(lines: &[&str], summary: &str) -> bool {
    let matched = overlap(lines, summary);
    matched >= MIN_LINES && matched * 2 > lines.len()
}

/// The day a finished block is a record of, when that is not the day it
/// was captured on. Two rules: the reference points at a file this app
/// wrote for another day, or the body repeats an earlier day's summary
/// back at us.
pub fn detect(folder: &Path, block: &Block) -> Option<NaiveDate> {
    let today = block.start.date_naive();
    for reference in [block.document.as_deref(), block.url.as_deref()]
        .into_iter()
        .flatten()
    {
        if let Some(date) = date_in_reference(reference) {
            if date != today {
                return Some(date);
            }
        }
    }
    let lines: Vec<&str> = block
        .lines
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect();
    if lines.len() < MIN_LINES {
        return None;
    }
    for back in 1..=LOOK_BACK_DAYS {
        let date = today - Duration::days(back);
        let Ok(summary) = std::fs::read_to_string(crate::summarise::summary_path(folder, date))
        else {
            continue;
        };
        if is_replay_of(&lines, &summary) {
            return Some(date);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, TimeZone};
    use tempfile::tempdir;

    fn block(document: Option<&str>, url: Option<&str>, lines: &[&str]) -> Block {
        Block {
            app: "Zed".to_string(),
            title: Some("2026-08-28.md".to_string()),
            document: document.map(str::to_string),
            url: url.map(str::to_string),
            start: Local.with_ymd_and_hms(2026, 9, 2, 9, 14, 0).unwrap(),
            end: Local.with_ymd_and_hms(2026, 9, 2, 9, 41, 0).unwrap(),
            lines: lines.iter().map(|s| s.to_string()).collect(),
            headings_only: false,
        }
    }

    fn folder_with_summary(date: NaiveDate, text: &str) -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(crate::summarise::summaries_dir(dir.path())).unwrap();
        std::fs::write(crate::summarise::summary_path(dir.path(), date), text).unwrap();
        dir
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn a_summary_path_for_another_day_is_a_replay() {
        let dir = tempdir().unwrap();
        let block = block(Some("/Users/x/Capture/Summaries/2026-08-28.md"), None, &[]);
        assert_eq!(detect(dir.path(), &block), Some(date(2026, 8, 28)));
    }

    #[test]
    fn a_kb_path_for_another_day_is_a_replay() {
        let dir = tempdir().unwrap();
        let block = block(
            None,
            Some("file:///x/Capture/KB/2026-08-30/threads.md"),
            &[],
        );
        assert_eq!(detect(dir.path(), &block), Some(date(2026, 8, 30)));
    }

    #[test]
    fn todays_own_summary_is_not_a_replay() {
        let dir = tempdir().unwrap();
        let block = block(Some("/Users/x/Capture/Summaries/2026-09-02.md"), None, &[]);
        assert_eq!(detect(dir.path(), &block), None);
    }

    #[test]
    fn a_body_repeating_an_earlier_summary_is_a_replay() {
        let dir = folder_with_summary(
            date(2026, 8, 28),
            "# A day of plumbing\n\nShipped the writer.\nFixed the segmenter.\nRead the Tauri docs.\n",
        );
        let block = block(
            None,
            None,
            &[
                "Shipped the writer.",
                "Fixed the segmenter.",
                "Read the Tauri docs.",
                "2026-08-28.md, Zed",
            ],
        );
        assert_eq!(detect(dir.path(), &block), Some(date(2026, 8, 28)));
    }

    #[test]
    fn an_overlap_of_half_or_less_is_not_a_replay() {
        let dir = folder_with_summary(
            date(2026, 8, 28),
            "# A day of plumbing\n\nShipped the writer.\nFixed the segmenter.\nRead the Tauri docs.\n",
        );
        let block = block(
            None,
            None,
            &[
                "Shipped the writer.",
                "Fixed the segmenter.",
                "Read the Tauri docs.",
                "fn append_block",
                "fn render_block",
                "fn strip_tracking",
            ],
        );
        assert_eq!(detect(dir.path(), &block), None);
    }

    #[test]
    fn fewer_than_three_matching_lines_is_not_a_replay() {
        let dir = folder_with_summary(
            date(2026, 8, 28),
            "# A day of plumbing\n\nShipped the writer.\nFixed the segmenter.\n",
        );
        let block = block(None, None, &["Shipped the writer.", "Fixed the segmenter."]);
        assert_eq!(detect(dir.path(), &block), None);
    }

    #[test]
    fn the_most_recent_matching_day_wins() {
        let text = "# A day\n\nShipped the writer.\nFixed the segmenter.\nRead the Tauri docs.\n";
        let dir = folder_with_summary(date(2026, 8, 28), text);
        std::fs::write(
            crate::summarise::summary_path(dir.path(), date(2026, 9, 1)),
            text,
        )
        .unwrap();
        let block = block(
            None,
            None,
            &[
                "Shipped the writer.",
                "Fixed the segmenter.",
                "Read the Tauri docs.",
            ],
        );
        assert_eq!(detect(dir.path(), &block), Some(date(2026, 9, 1)));
    }
}
