use crate::writer::{self, DayFile};
use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::Path;

/// One day as the window sees it. `has_capture` and `has_summary` are the
/// two marks the calendar draws; `bytes` is the sum of the three day files;
/// `title` is the summary's own one-line name for the day.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DayEntry {
    pub date: NaiveDate,
    pub has_capture: bool,
    pub has_summary: bool,
    pub has_kb: bool,
    pub bytes: u64,
    pub title: Option<String>,
}

fn entry(folder: &Path, date: NaiveDate) -> DayEntry {
    let summary = std::fs::read_to_string(crate::summarise::summary_path(folder, date)).ok();
    let bytes = DayFile::all()
        .iter()
        .filter_map(|file| std::fs::metadata(file.path(folder, date)).ok())
        .map(|m| m.len())
        .sum();
    DayEntry {
        date,
        has_capture: DayFile::Apps.path(folder, date).is_file(),
        has_summary: summary.is_some(),
        has_kb: false, // Task 9: crate::ingest::has_kb(folder, date)
        bytes,
        title: summary.as_deref().and_then(crate::summarise::title_of),
    }
}

fn known_dates(folder: &Path) -> BTreeSet<NaiveDate> {
    let mut dates: BTreeSet<NaiveDate> = crate::summarise::list_captured(folder)
        .into_iter()
        .collect();
    dates.extend(crate::summarise::list_summarised(folder));
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

pub fn read_day(folder: &Path, date: NaiveDate, file: DayFile) -> Option<String> {
    std::fs::read_to_string(file.path(folder, date)).ok()
}

pub fn read_summary(folder: &Path, date: NaiveDate) -> Option<String> {
    std::fs::read_to_string(crate::summarise::summary_path(folder, date)).ok()
}

/// The `## ` headings of apps.md, one per line: the day's clock.
pub fn timeline(folder: &Path, date: NaiveDate) -> Option<String> {
    let text = read_day(folder, date, DayFile::Apps)?;
    let mut out = String::new();
    for line in text.lines().filter(|l| l.starts_with("## ")) {
        out.push_str(line);
        out.push('\n');
    }
    Some(out)
}

fn minutes(hhmm: &str) -> Option<u32> {
    let (h, m) = hhmm.split_once(':')?;
    Some(h.parse::<u32>().ok()? * 60 + m.parse::<u32>().ok()?)
}

/// `(start, end)` in minutes since midnight for every heading. An end
/// before its start crossed midnight and is carried past 1440.
pub fn spans(timeline: &str) -> Vec<(u32, u32)> {
    timeline
        .lines()
        .filter_map(|line| {
            let (start, end, _, _) = parse_heading(line)?;
            let s = minutes(&start)?;
            let mut e = minutes(&end)?;
            if e < s {
                e += 24 * 60;
            }
            Some((s, e))
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UrlTotal {
    pub url: String,
    pub domain: String,
    pub title: String,
    pub dwell_secs: u64,
    pub visits: u32,
    pub first: String,
    pub last: String,
}

fn unescape_cell(cell: &str) -> String {
    cell.replace("\\|", "|")
}

/// Splits a table row on unescaped pipes. The leading and trailing
/// separators produce empty first and last cells, which are dropped.
fn row_cells(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'|') {
            current.push_str("\\|");
            chars.next();
        } else if c == '|' {
            cells.push(unescape_cell(current.trim()));
            current.clear();
        } else {
            current.push(c);
        }
    }
    cells.push(unescape_cell(current.trim()));
    if cells.len() >= 2 {
        cells.remove(0);
        cells.pop();
    }
    cells
}

pub fn website_totals(folder: &Path, date: NaiveDate) -> Vec<UrlTotal> {
    let Some(text) = read_day(folder, date, DayFile::Websites) else {
        return Vec::new();
    };
    let mut totals: Vec<UrlTotal> = Vec::new();
    let mut longest: Vec<u64> = Vec::new();
    for line in text
        .lines()
        .filter(|l| l.starts_with("| ") && !l.starts_with("| start") && !l.starts_with("| ---"))
    {
        let cells = row_cells(line);
        if cells.len() != 6 {
            continue;
        }
        let (Some(s), Some(e)) = (minutes(&cells[0]), minutes(&cells[1])) else {
            continue;
        };
        let e = if e < s { e + 24 * 60 } else { e };
        let dwell = u64::from(e - s) * 60;
        let url = cells[5].clone();
        let key_title = cells[4].clone();
        let position = totals.iter().position(|t| {
            if url.is_empty() {
                t.url.is_empty() && t.title == key_title
            } else {
                t.url == url
            }
        });
        match position {
            Some(i) => {
                totals[i].dwell_secs += dwell;
                totals[i].visits += 1;
                totals[i].last = cells[1].clone();
                if dwell > longest[i] {
                    longest[i] = dwell;
                    totals[i].title = key_title;
                }
            }
            None => {
                totals.push(UrlTotal {
                    url,
                    domain: cells[3].clone(),
                    title: key_title,
                    dwell_secs: dwell,
                    visits: 1,
                    first: cells[0].clone(),
                    last: cells[1].clone(),
                });
                longest.push(dwell);
            }
        }
    }
    totals.sort_by(|a, b| b.dwell_secs.cmp(&a.dwell_secs).then(a.first.cmp(&b.first)));
    totals
}

pub fn render_totals(totals: &[UrlTotal]) -> String {
    let mut out = String::from(
        "| domain | title | dwell | visits | first | last | url |\n| --- | --- | --- | --- | --- | --- | --- |\n",
    );
    for t in totals {
        out.push_str(&format!(
            "| {} | {} | {}m | {} | {} | {} | {} |\n",
            writer::escape_cell(&t.domain),
            writer::escape_cell(&t.title),
            t.dwell_secs / 60,
            t.visits,
            t.first,
            t.last,
            writer::escape_cell(&t.url),
        ));
    }
    out
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
    pub routed: Option<String>,
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
                    routed: None,
                    lines: Vec::new(),
                });
            }
            continue;
        }
        let Some(block) = blocks.last_mut() else {
            continue;
        };
        if let Some(path) = line.strip_prefix("file: ") {
            block.file = Some(path.to_string());
        } else if let Some(url) = line.strip_prefix("url: ") {
            block.url = Some(url.to_string());
        } else if let Some(name) = line.strip_prefix("routed: ") {
            block.routed = Some(name.to_string());
        } else if !line.trim().is_empty() {
            block.lines.push(line.to_string());
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::DayFile;
    use tempfile::tempdir;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn write(dir: &Path, date: NaiveDate, file: DayFile, text: &str) {
        let path = file.path(dir, date);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    const APPS: &str = "---\ndate: 2026-08-27\nkind: apps\n---\n\n## 09:00\u{2013}09:30 \u{00b7} Zed \u{00b7} writer.rs\n\nfile: /x/writer.rs\n\nfn a\n\n## 09:30\u{2013}09:41 \u{00b7} Arc \u{00b7} Tauri\n\nrouted: websites\n\n## 23:50\u{2013}00:10 \u{00b7} Slack \u{00b7} #x\n\nrouted: messages\n";

    const WEBSITES: &str = "---\ndate: 2026-08-27\nkind: websites\n---\n\n| start | end | app | domain | title | url |\n| --- | --- | --- | --- | --- | --- |\n| 09:30 | 09:41 | Arc | v2.tauri.app | Tauri | https://v2.tauri.app/ |\n| 10:00 | 10:05 | Arc | v2.tauri.app | Tauri again | https://v2.tauri.app/ |\n| 10:05 | 10:06 | Arc |  | Loading \\| page |  |\n| 10:06 | 10:07 | Arc |  | Loading \\| page |  |\n";

    fn folder() -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        write(dir.path(), date(2026, 8, 27), DayFile::Apps, APPS);
        write(dir.path(), date(2026, 8, 27), DayFile::Websites, WEBSITES);
        write(dir.path(), date(2026, 8, 28), DayFile::Apps, "---\n---\n");
        std::fs::write(dir.path().join("2026-08-20.md"), "old").unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "x").unwrap();
        std::fs::create_dir_all(dir.path().join("Summaries")).unwrap();
        std::fs::write(
            dir.path().join("Summaries").join("2026-08-27.md"),
            "---\ndate: 2026-08-27\n---\n\n# A day of plumbing\n\nprose",
        )
        .unwrap();
        dir
    }

    #[test]
    fn list_days_reads_folders_and_ignores_flat_files() {
        let dir = folder();
        let days = list_days(dir.path());
        let dates: Vec<String> = days.iter().map(|d| d.date.to_string()).collect();
        assert_eq!(dates, vec!["2026-08-28", "2026-08-27"]);
        let first = &days[1];
        assert!(first.has_capture);
        assert!(first.has_summary);
        assert_eq!(first.bytes, (APPS.len() + WEBSITES.len()) as u64);
        assert_eq!(first.title.as_deref(), Some("A day of plumbing"));
    }

    #[test]
    fn read_day_returns_one_file() {
        let dir = folder();
        assert_eq!(
            read_day(dir.path(), date(2026, 8, 27), DayFile::Websites).unwrap(),
            WEBSITES
        );
        assert!(read_day(dir.path(), date(2026, 8, 27), DayFile::Messages).is_none());
    }

    #[test]
    fn timeline_is_headings_only() {
        let dir = folder();
        let out = timeline(dir.path(), date(2026, 8, 27)).unwrap();
        assert_eq!(out.lines().count(), 3);
        assert!(out.lines().all(|l| l.starts_with("## ")));
        assert!(!out.contains("routed:"));
    }

    #[test]
    fn spans_parse_minutes_and_cross_midnight() {
        let out =
            spans("## 09:00\u{2013}09:30 \u{00b7} Zed\n## 23:50\u{2013}00:10 \u{00b7} Slack\n");
        assert_eq!(out, vec![(540, 570), (1430, 1450)]);
    }

    #[test]
    fn website_totals_merge_by_url_and_rank_by_dwell() {
        let dir = folder();
        let totals = website_totals(dir.path(), date(2026, 8, 27));
        assert_eq!(totals.len(), 2);
        assert_eq!(totals[0].url, "https://v2.tauri.app/");
        assert_eq!(totals[0].dwell_secs, 16 * 60);
        assert_eq!(totals[0].visits, 2);
        assert_eq!(totals[0].title, "Tauri", "title of the longest visit");
        assert_eq!(
            (totals[0].first.as_str(), totals[0].last.as_str()),
            ("09:30", "10:05")
        );
        assert_eq!(
            totals[1].title, "Loading | page",
            "empty-url rows merge by title, unescaped"
        );
        assert_eq!(totals[1].visits, 2);
    }

    #[test]
    fn render_totals_is_a_pipe_table_with_dwell_in_minutes() {
        let dir = folder();
        let out = render_totals(&website_totals(dir.path(), date(2026, 8, 27)));
        assert!(out.starts_with("| domain | title | dwell | visits | first | last | url |\n| --- | --- | --- | --- | --- | --- | --- |\n"));
        assert!(out.contains(
            "| v2.tauri.app | Tauri | 16m | 2 | 09:30 | 10:05 | https://v2.tauri.app/ |\n"
        ));
        assert!(out.contains("|  | Loading \\| page |"));
    }

    #[test]
    fn parse_blocks_keeps_routed_out_of_the_body() {
        let blocks = parse_blocks(APPS);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[1].routed.as_deref(), Some("websites"));
        assert!(blocks[1].lines.is_empty());
        assert_eq!(blocks[0].file.as_deref(), Some("/x/writer.rs"));
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
        assert!(parse_blocks(DAY)
            .iter()
            .all(|b| !b.app.contains("captured_by")));
    }

    #[test]
    fn an_empty_or_malformed_file_yields_no_blocks() {
        assert!(parse_blocks("").is_empty());
        assert!(parse_blocks("## not a heading we wrote\n\nbody\n").is_empty());
        assert!(parse_blocks("## 09:14 \u{00b7} Linear\n").is_empty());
    }

    #[test]
    fn a_folder_that_does_not_exist_lists_nothing_rather_than_panicking() {
        assert!(list_days(std::path::Path::new("/nope/not/here")).is_empty());
    }
}
