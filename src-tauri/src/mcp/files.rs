use chrono::NaiveDate;
use serde::Serialize;
use std::path::Path;

#[derive(Debug)]
pub enum FileError {
    NoCapture(NaiveDate),
    NoSummary(NaiveDate),
    NoLedger(NaiveDate),
    BadTime(String),
    NoFolder,
}

impl std::fmt::Display for FileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileError::NoCapture(date) => write!(f, "There is no capture for {date}."),
            FileError::NoSummary(date) => write!(
                f,
                "There is no summary for {date} yet. Call summarise_day to generate one."
            ),
            FileError::NoLedger(date) => write!(f, "There are no ledger entries for {date}."),
            FileError::BadTime(value) => {
                write!(
                    f,
                    "{value} is not a time. Use 24-hour HH:MM, for example 09:30."
                )
            }
            FileError::NoFolder => write!(
                f,
                "No capture folder is set. Open Ambient Context and choose one."
            ),
        }
    }
}

pub fn list_days(folder: &Path) -> serde_json::Value {
    let days: Vec<serde_json::Value> = crate::days::list_days(folder)
        .into_iter()
        .map(|day| {
            serde_json::json!({
                "date": day.date.to_string(),
                "has_capture": day.has_capture,
                "has_summary": day.has_summary,
                "bytes": day.bytes,
                "title": day.title,
            })
        })
        .collect();
    serde_json::json!({ "folder": folder.to_string_lossy(), "days": days })
}

fn valid_time(value: &str) -> Result<&str, FileError> {
    let bytes = value.as_bytes();
    let shaped = bytes.len() == 5
        && bytes[2] == b':'
        && bytes[0].is_ascii_digit()
        && bytes[1].is_ascii_digit()
        && bytes[3].is_ascii_digit()
        && bytes[4].is_ascii_digit();
    if shaped && &value[0..2] <= "23" && &value[3..5] <= "59" {
        Ok(value)
    } else {
        Err(FileError::BadTime(value.to_string()))
    }
}

/// The whole day file, or the blocks whose heading start time falls in
/// [from, to). The slice is cut out of the file's own bytes rather than
/// re-rendered from parsed blocks, so what an agent reads is exactly what the
/// Raw view shows and exactly what is on disk.
pub fn read_day(
    folder: &Path,
    date: NaiveDate,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<String, FileError> {
    let text = crate::days::read_day(folder, date).ok_or(FileError::NoCapture(date))?;
    if from.is_none() && to.is_none() {
        return Ok(text);
    }
    let from = from.map(valid_time).transpose()?.unwrap_or("00:00");
    let to = to.map(valid_time).transpose()?.unwrap_or("24:00");

    let mut out = String::new();
    let mut keeping = false;
    for line in text.lines() {
        if let Some(start) = heading_time(line) {
            keeping = start >= from && start < to;
        }
        if keeping {
            out.push_str(line);
            out.push('\n');
        }
    }
    Ok(out)
}

/// The start time of a block heading, which writer::render_block emits as
/// `## HH:MM–HH:MM · App · Title`.
fn heading_time(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("## ")?;
    // Captured window text is written verbatim into block bodies, so a
    // heading can begin with any character at all. Slicing five bytes off
    // one that begins mid-character panics the whole server process, so
    // ask for the five bytes rather than taking them.
    let start = rest.get(0..5)?;
    if valid_time(start).is_ok() {
        Some(start)
    } else {
        None
    }
}

pub fn read_summary(folder: &Path, date: NaiveDate) -> Result<String, FileError> {
    crate::days::read_summary(folder, date).ok_or(FileError::NoSummary(date))
}

pub fn read_ledger(folder: &Path, date: NaiveDate) -> Result<String, FileError> {
    std::fs::read_to_string(folder.join("Ledger").join(format!("{date}.md")))
        .map_err(|_| FileError::NoLedger(date))
}

#[derive(Debug, Serialize)]
pub struct Hit {
    pub date: String,
    pub layer: &'static str,
    pub line: usize,
    pub text: String,
    pub context: Vec<String>,
}

/// Case-insensitive substring search across day files and summaries. No index,
/// no embeddings: the record is a few megabytes of markdown and a linear scan
/// is faster than anything that would need maintaining.
pub fn search_record(folder: &Path, query: &str, limit: usize) -> Vec<Hit> {
    let needle = query.to_lowercase();
    let mut hits = Vec::new();
    for day in crate::days::list_days(folder) {
        let date = day.date.to_string();
        let sources = [
            ("day", folder.join(format!("{date}.md"))),
            (
                "summary",
                folder.join("Summaries").join(format!("{date}.md")),
            ),
        ];
        for (layer, path) in sources {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let lines: Vec<&str> = text.lines().collect();
            for (index, line) in lines.iter().enumerate() {
                if hits.len() >= limit {
                    return hits;
                }
                if !line.to_lowercase().contains(&needle) {
                    continue;
                }
                let start = index.saturating_sub(2);
                let end = (index + 3).min(lines.len());
                hits.push(Hit {
                    date: date.clone(),
                    layer,
                    line: index + 1,
                    text: (*line).to_string(),
                    context: lines[start..end]
                        .iter()
                        .map(|line| (*line).to_string())
                        .collect(),
                });
            }
        }
    }
    hits
}

pub fn list_rules(config_dir: &Path) -> serde_json::Value {
    let set = crate::rules::load(config_dir);
    serde_json::json!({
        "rules": set.rules,
        "built_ins": crate::rules::built_ins(),
        "note": "Built-in protections are shown so you can see what is never recorded. \
                 They cannot be changed from any surface, including this one.",
    })
}

pub fn get_prompt(config_dir: &Path) -> serde_json::Value {
    serde_json::json!({
        "text": crate::prompt::current(config_dir),
        "customised": crate::prompt::is_customised(config_dir),
    })
}

pub fn get_config(config_dir: &Path) -> serde_json::Value {
    let settings = crate::settings::read_from(&config_dir.join("settings.json"));
    let mut value = serde_json::to_value(&settings).unwrap_or(serde_json::Value::Null);
    if let serde_json::Value::Object(map) = &mut value {
        map.insert(
            "version".into(),
            serde_json::json!(env!("CARGO_PKG_VERSION")),
        );
        map.insert(
            "settable_keys".into(),
            serde_json::json!(crate::control::writes::SETTABLE_KEYS),
        );
        map.insert(
            "prompt_customised".into(),
            serde_json::json!(crate::prompt::is_customised(config_dir)),
        );
    }
    value
}

/// The capture folder, read from settings rather than assumed, because every
/// read tool needs it and the app may not be running to be asked.
pub fn folder_from(config_dir: &Path) -> Result<std::path::PathBuf, FileError> {
    crate::settings::read_from(&config_dir.join("settings.json"))
        .folder
        .ok_or(FileError::NoFolder)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder_with_a_day() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("2026-08-30.md"),
            "---\ndate: 2026-08-30\n---\n\n\
             ## 09:00\u{2013}09:20 \u{b7} Safari \u{b7} Postgres docs\n\n\
             url: https://www.example.org/docs\n\n\
             Index-only scans need a visibility map.\n\n\
             ## 11:05\u{2013}11:40 \u{b7} Xcode \u{b7} Reader.swift\n\n\
             The accessibility walk is depth first.\n\n\
             ## 16:00\u{2013}16:30 \u{b7} Slack \u{b7} general\n\n\
             Standup moved to nine.\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("Summaries")).unwrap();
        std::fs::write(
            dir.path().join("Summaries").join("2026-08-30.md"),
            "---\ndate: 2026-08-30\n---\n\n## What happened\n\nRead the Postgres docs (09:00 to 09:20).\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("Ledger")).unwrap();
        std::fs::write(
            dir.path().join("Ledger").join("2026-08-30.md"),
            "## 06:02 summarise_day\n\ntrigger: schedule\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn a_multibyte_heading_is_not_a_time_rather_than_a_panic() {
        // Captured text goes into block bodies verbatim, so a line that
        // starts with "## " and a non-ASCII character reaches this code.
        assert_eq!(
            heading_time("## \u{65e5}\u{672c}\u{8a9e}\u{306e}\u{898b}\u{51fa}\u{3057}"),
            None
        );
        assert_eq!(heading_time("## \u{2014} 09:00 notes"), None);
        assert_eq!(heading_time("## \u{4e2d}09:00"), None);
        assert_eq!(heading_time("## 09"), None);
        assert_eq!(
            heading_time("## 09:00\u{2013}09:20 \u{b7} Safari"),
            Some("09:00")
        );
    }

    #[test]
    fn a_slice_of_a_day_containing_a_multibyte_heading_still_answers() {
        let dir = tempfile::tempdir().unwrap();
        let day = [
            "## 09:00\u{2013}09:20 \u{b7} Pages \u{b7} Notes",
            "",
            "## \u{65e5}\u{672c}\u{8a9e}\u{306e}\u{898b}\u{51fa}\u{3057}",
            "",
            "## 16:00\u{2013}16:30 \u{b7} Slack \u{b7} general",
            "",
            "Standup moved to nine.",
        ]
        .join("\n");
        std::fs::write(dir.path().join("2026-08-30.md"), day).unwrap();
        let text = read_day(dir.path(), date(2026, 8, 30), Some("08:00"), Some("10:00")).unwrap();
        assert!(text.contains("Pages"));
        // The heading that is not a time belongs to the block above it.
        assert!(text.contains("\u{65e5}\u{672c}\u{8a9e}"));
        assert!(!text.contains("Slack"));
    }

    #[test]
    fn a_whole_day_comes_back_verbatim() {
        let dir = folder_with_a_day();
        let text = read_day(dir.path(), date(2026, 8, 30), None, None).unwrap();
        assert!(text.starts_with("---\ndate: 2026-08-30"));
        assert!(text.contains("Standup moved to nine."));
    }

    #[test]
    fn a_time_slice_keeps_whole_blocks_inside_the_range() {
        let dir = folder_with_a_day();
        let text = read_day(dir.path(), date(2026, 8, 30), Some("10:00"), Some("12:00")).unwrap();
        assert!(text.contains("Xcode"));
        assert!(!text.contains("Safari"), "the 09:00 block leaked in");
        assert!(!text.contains("Slack"), "the 16:00 block leaked in");
    }

    #[test]
    fn a_slice_is_half_open_so_a_block_starting_at_to_is_excluded() {
        let dir = folder_with_a_day();
        let text = read_day(dir.path(), date(2026, 8, 30), Some("09:00"), Some("11:05")).unwrap();
        assert!(text.contains("Safari"));
        assert!(!text.contains("Xcode"));
    }

    #[test]
    fn a_slice_drops_the_frontmatter_because_it_is_not_a_block() {
        let dir = folder_with_a_day();
        let text = read_day(dir.path(), date(2026, 8, 30), Some("09:00"), None).unwrap();
        assert!(text.starts_with("## 09:00"));
    }

    #[test]
    fn a_bad_time_is_rejected_with_the_expected_form() {
        let dir = folder_with_a_day();
        let error = read_day(dir.path(), date(2026, 8, 30), Some("9am"), None).unwrap_err();
        assert!(matches!(error, FileError::BadTime(_)));
    }

    #[test]
    fn a_missing_day_is_not_found_rather_than_empty() {
        let dir = folder_with_a_day();
        assert!(matches!(
            read_day(dir.path(), date(2026, 8, 29), None, None),
            Err(FileError::NoCapture(_))
        ));
    }

    #[test]
    fn search_is_case_insensitive_and_reports_the_layer() {
        let dir = folder_with_a_day();
        let hits = search_record(dir.path(), "POSTGRES", 20);
        assert_eq!(hits.len(), 2, "{hits:?}");
        assert!(hits.iter().any(|hit| hit.layer == "day"));
        assert!(hits.iter().any(|hit| hit.layer == "summary"));
    }

    #[test]
    fn a_hit_carries_its_date_line_number_and_surrounding_lines() {
        let dir = folder_with_a_day();
        let hits = search_record(dir.path(), "depth first", 20);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].date, "2026-08-30");
        assert!(hits[0].line > 0);
        assert!(hits[0].context.iter().any(|line| line.contains("Xcode")));
    }

    #[test]
    fn search_stops_at_the_limit() {
        let dir = folder_with_a_day();
        assert_eq!(search_record(dir.path(), "the", 1).len(), 1);
    }

    #[test]
    fn the_ledger_for_a_day_comes_back_whole() {
        let dir = folder_with_a_day();
        assert!(read_ledger(dir.path(), date(2026, 8, 30))
            .unwrap()
            .contains("summarise_day"));
    }

    #[test]
    fn a_day_with_no_ledger_says_so_rather_than_failing() {
        let dir = folder_with_a_day();
        assert!(matches!(
            read_ledger(dir.path(), date(2026, 8, 29)),
            Err(FileError::NoLedger(_))
        ));
    }

    #[test]
    fn get_config_reports_the_version_and_no_retention_key() {
        let dir = tempfile::tempdir().unwrap();
        let value = get_config(dir.path());
        assert_eq!(
            value["version"],
            serde_json::json!(env!("CARGO_PKG_VERSION"))
        );
        assert!(value.get("retention").is_none());
        assert!(value["settable_keys"]
            .as_array()
            .unwrap()
            .iter()
            .any(|key| key == "schedule_hhmm"));
    }

    fn date(y: i32, m: u32, d: u32) -> chrono::NaiveDate {
        chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }
}
