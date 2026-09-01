use regex::Regex;
use std::sync::OnceLock;

/// A day summary should be shorter than the day it describes. The prompt
/// asks for under 700 words; this is the backstop against a model that
/// pastes the input back.
pub const MAX_SUMMARY_LINES: usize = 200;

#[derive(Debug, PartialEq)]
pub enum Invalid {
    Empty,
    NoFrontmatter,
    MissingField(&'static str),
    NoSections,
    /// The summary makes claims with no time ranges pointing back at the
    /// day file. A summary that cannot be checked against the record is an
    /// opinion, and the record is the only thing this product sells.
    NoCitations,
    TooLong {
        lines: usize,
        max: usize,
    },
}

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Invalid::Empty => write!(f, "the agent returned nothing"),
            Invalid::NoFrontmatter => write!(f, "the summary has no frontmatter block"),
            Invalid::MissingField(field) => write!(f, "the summary is missing '{field}'"),
            Invalid::NoSections => write!(f, "the summary has no sections"),
            Invalid::NoCitations => {
                write!(f, "the summary cites no time ranges from the day file")
            }
            Invalid::TooLong { lines, max } => {
                write!(
                    f,
                    "the summary is {lines} lines, over the {max} line budget"
                )
            }
        }
    }
}

/// Strips a wrapping code fence if the model added one. Left as a separate
/// step so validation reads the content either way.
pub fn unfence(text: &str) -> &str {
    let trimmed = text.trim();
    if !trimmed.starts_with("```") {
        return trimmed;
    }
    let after_open = match trimmed.find('\n') {
        Some(index) => &trimmed[index + 1..],
        None => return trimmed,
    };
    match after_open.rfind("```") {
        Some(index) => after_open[..index].trim(),
        None => after_open.trim(),
    }
}

/// `09:14-09:41` or `09:14–09:41`. Both dashes appear in practice: the day
/// file's own headings use an en dash and models copy either.
fn citation() -> &'static Regex {
    static CITATION: OnceLock<Regex> = OnceLock::new();
    CITATION.get_or_init(|| Regex::new(r"\b\d{1,2}:\d{2}\s*[-\x{2013}]\s*\d{1,2}:\d{2}\b").unwrap())
}

pub fn validate(text: &str, max_lines: usize) -> Result<(), Invalid> {
    let body = unfence(text);
    if body.trim().is_empty() {
        return Err(Invalid::Empty);
    }

    let lines: Vec<&str> = body.lines().collect();
    if lines.len() > max_lines {
        return Err(Invalid::TooLong {
            lines: lines.len(),
            max: max_lines,
        });
    }

    if lines.first().map(|l| l.trim()) != Some("---") {
        return Err(Invalid::NoFrontmatter);
    }
    let close = lines
        .iter()
        .skip(1)
        .position(|l| l.trim() == "---")
        .ok_or(Invalid::NoFrontmatter)?
        + 1;

    let frontmatter = lines[1..close].join("\n");
    for field in ["date", "type"] {
        if !frontmatter
            .lines()
            .any(|line| line.trim_start().starts_with(&format!("{field}:")))
        {
            return Err(Invalid::MissingField(field));
        }
    }

    if !lines[close..].iter().any(|l| l.starts_with("## ")) {
        return Err(Invalid::NoSections);
    }
    if !lines[close..].iter().any(|l| l.trim() == "## Reasoning") {
        return Err(Invalid::MissingField("Reasoning"));
    }

    // The body only: the frontmatter's `date:` line must not count as a
    // citation, and neither must anything above the closing fence.
    if !citation().is_match(&lines[close..].join("\n")) {
        return Err(Invalid::NoCitations);
    }

    Ok(())
}

use chrono::NaiveDate;
use std::path::{Path, PathBuf};

/// The prompt shipped with this version. A user-supplied prompt replaces it
/// entirely; this copy is never edited in place, so an update can improve it
/// without touching anyone's own file.
#[cfg(test)]
pub const BUNDLED_PROMPT: &str = include_str!("../prompts/day-context.md");

pub fn summaries_dir(folder: &Path) -> PathBuf {
    folder.join("Summaries")
}

pub fn summary_path(folder: &Path, date: NaiveDate) -> PathBuf {
    summaries_dir(folder).join(format!("{}.md", date.format("%Y-%m-%d")))
}

pub fn build_prompt(template: &str, date: NaiveDate, day_markdown: &str) -> String {
    template
        .replace("{{DATE}}", &date.format("%Y-%m-%d").to_string())
        .replace("{{DAY_FILE}}", day_markdown)
}

pub fn write_summary(folder: &Path, date: NaiveDate, body: &str) -> std::io::Result<()> {
    let dir = summaries_dir(folder);
    std::fs::create_dir_all(&dir)?;
    let mut text = unfence(body).to_string();
    if !text.ends_with('\n') {
        text.push('\n');
    }
    std::fs::write(summary_path(folder, date), text)
}

fn dates_in(dir: &Path) -> Vec<NaiveDate> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    let mut dates: Vec<NaiveDate> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let stem = name.strip_suffix(".md")?;
            NaiveDate::parse_from_str(stem, "%Y-%m-%d").ok()
        })
        .collect();
    dates.sort();
    dates
}

pub fn list_captured(folder: &Path) -> Vec<NaiveDate> {
    dates_in(folder)
}

pub fn list_summarised(folder: &Path) -> Vec<NaiveDate> {
    dates_in(&summaries_dir(folder))
}

/// The one-line title a summary gives its day, used in the Day header.
pub fn title_of(summary: &str) -> Option<String> {
    summary
        .lines()
        .find(|line| line.starts_with("# "))
        .map(|line| line[2..].trim().to_string())
}

/// The model's own account of its choices, lifted out of the summary so the
/// ledger can carry it without a second call. Testimony, not a trace: it can
/// be wrong in the same ways the summary can.
pub fn reasoning_of(summary: &str) -> Option<String> {
    let body = unfence(summary);
    let mut lines = body
        .lines()
        .skip_while(|line| line.trim() != "## Reasoning");
    lines.next()?;
    let text = lines
        .take_while(|line| !line.starts_with("## "))
        .collect::<Vec<&str>>()
        .join("\n")
        .trim()
        .to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> String {
        [
            "---",
            "date: 2026-08-28",
            "type: day-context",
            "generated_by: claude-opus-5",
            "---",
            "",
            "# A day of plumbing",
            "",
            "Narrative paragraph about the day.",
            "",
            "## Sessions",
            "09:00-11:00 building the thing.",
            "",
            "## Reasoning",
            "Treated the long Linear block as the spine of the day.",
        ]
        .join("\n")
    }

    #[test]
    fn accepts_a_well_formed_summary() {
        assert!(validate(&good(), MAX_SUMMARY_LINES).is_ok());
    }

    #[test]
    fn rejects_empty_output() {
        assert!(matches!(
            validate("   \n  ", MAX_SUMMARY_LINES),
            Err(Invalid::Empty)
        ));
    }

    #[test]
    fn rejects_output_with_no_frontmatter() {
        let text = "# A day\n\nSome prose.\n\n## Sessions\n09:00-11:00 things";
        assert!(matches!(
            validate(text, MAX_SUMMARY_LINES),
            Err(Invalid::NoFrontmatter)
        ));
    }

    #[test]
    fn rejects_frontmatter_missing_the_type_field() {
        let text = good().replace("type: day-context\n", "");
        assert!(matches!(
            validate(&text, MAX_SUMMARY_LINES),
            Err(Invalid::MissingField("type"))
        ));
    }

    #[test]
    fn rejects_frontmatter_missing_the_date_field() {
        let text = good().replace("date: 2026-08-28\n", "");
        assert!(matches!(
            validate(&text, MAX_SUMMARY_LINES),
            Err(Invalid::MissingField("date"))
        ));
    }

    #[test]
    fn rejects_output_with_no_sections() {
        let text = "---\ndate: 2026-08-28\ntype: day-context\n---\n\nJust prose, no headings.";
        assert!(matches!(
            validate(text, MAX_SUMMARY_LINES),
            Err(Invalid::NoSections)
        ));
    }

    #[test]
    fn rejects_a_summary_that_never_states_its_reasoning() {
        let text = good().replace("## Reasoning", "## Notes");
        assert!(matches!(
            validate(&text, MAX_SUMMARY_LINES),
            Err(Invalid::MissingField("Reasoning"))
        ));
    }

    #[test]
    fn rejects_a_summary_with_no_time_ranges() {
        // A summary that cannot be checked against the record is an
        // opinion, and the record is the only thing this product sells.
        let text = [
            "---",
            "date: 2026-08-28",
            "type: day-context",
            "generated_by: claude-opus-5",
            "---",
            "",
            "# A day of plumbing",
            "",
            "## Sessions",
            "Spent the day building the thing and it went well.",
            "",
            "## Reasoning",
            "Wrote it from the headings.",
        ]
        .join("\n");
        assert!(matches!(
            validate(&text, MAX_SUMMARY_LINES),
            Err(Invalid::NoCitations)
        ));
    }

    #[test]
    fn accepts_ranges_written_with_either_dash() {
        for sep in ["-", "\u{2013}"] {
            let text = good().replace("09:00-11:00", &format!("09:00{sep}11:00"));
            assert!(
                validate(&text, MAX_SUMMARY_LINES).is_ok(),
                "failed on {sep:?}"
            );
        }
    }

    #[test]
    fn frontmatter_dates_are_not_mistaken_for_citations() {
        // `date: 2026-08-28` must not satisfy the citation check on its own.
        let text = [
            "---",
            "date: 2026-08-28",
            "type: day-context",
            "generated_by: claude-opus-5",
            "---",
            "",
            "# A day",
            "",
            "## Sessions",
            "No ranges here at all.",
            "",
            "## Reasoning",
            "Nothing to add.",
        ]
        .join("\n");
        assert!(matches!(
            validate(&text, MAX_SUMMARY_LINES),
            Err(Invalid::NoCitations)
        ));
    }

    #[test]
    fn rejects_output_over_the_line_budget() {
        let mut text = good();
        for i in 0..50 {
            text.push_str(&format!("\nline {i}"));
        }
        assert!(matches!(
            validate(&text, 20),
            Err(Invalid::TooLong { max: 20, .. })
        ));
    }

    #[test]
    fn a_model_that_wraps_output_in_a_code_fence_still_passes() {
        // Every CLI does this at least sometimes. Rejecting it would fail
        // for a formatting habit rather than a content problem.
        let text = format!("```markdown\n{}\n```", good());
        assert!(validate(&text, MAX_SUMMARY_LINES).is_ok());
    }

    use chrono::NaiveDate;
    use tempfile::tempdir;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn summaries_live_in_a_summaries_folder_named_by_date() {
        let path = summary_path(std::path::Path::new("/tmp/ac"), date(2026, 8, 28));
        assert_eq!(
            path,
            std::path::PathBuf::from("/tmp/ac/Summaries/2026-08-28.md")
        );
    }

    #[test]
    fn the_prompt_carries_the_date_and_the_whole_day_file() {
        let out = build_prompt(
            "Date: {{DATE}}\nBody:\n{{DAY_FILE}}",
            date(2026, 8, 28),
            "## 09:00 block",
        );
        assert_eq!(out, "Date: 2026-08-28\nBody:\n## 09:00 block");
    }

    #[test]
    fn the_bundled_prompt_carries_both_placeholders_and_the_reasoning_section() {
        assert!(BUNDLED_PROMPT.contains("{{DATE}}"));
        assert!(BUNDLED_PROMPT.contains("{{DAY_FILE}}"));
        assert!(BUNDLED_PROMPT.contains("## Reasoning"));
    }

    #[test]
    fn writing_a_summary_creates_the_folder_and_strips_any_code_fence() {
        let dir = tempdir().unwrap();
        write_summary(
            dir.path(),
            date(2026, 8, 28),
            "```markdown\n---\nx\n---\n```",
        )
        .unwrap();
        let written = std::fs::read_to_string(summary_path(dir.path(), date(2026, 8, 28))).unwrap();
        assert_eq!(written, "---\nx\n---\n");
    }

    #[test]
    fn listing_finds_day_files_and_ignores_everything_else() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("2026-08-27.md"), "x").unwrap();
        std::fs::write(dir.path().join("2026-08-28.md"), "x").unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "x").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "x").unwrap();
        std::fs::create_dir_all(dir.path().join("Summaries")).unwrap();
        assert_eq!(
            list_captured(dir.path()),
            vec![date(2026, 8, 27), date(2026, 8, 28)]
        );
    }

    #[test]
    fn listing_summaries_reads_the_summaries_folder_only() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("2026-08-27.md"), "x").unwrap();
        std::fs::create_dir_all(dir.path().join("Summaries")).unwrap();
        std::fs::write(dir.path().join("Summaries").join("2026-08-27.md"), "x").unwrap();
        assert_eq!(list_summarised(dir.path()), vec![date(2026, 8, 27)]);
    }

    #[test]
    fn listing_summaries_of_a_folder_without_one_is_empty_rather_than_an_error() {
        let dir = tempdir().unwrap();
        assert!(list_summarised(dir.path()).is_empty());
    }

    #[test]
    fn the_title_is_the_first_heading_after_the_frontmatter() {
        let text = "---\ndate: 2026-08-28\n---\n\n# A day of plumbing\n\nprose";
        assert_eq!(title_of(text), Some("A day of plumbing".to_string()));
    }

    #[test]
    fn a_summary_with_no_heading_has_no_title() {
        assert_eq!(title_of("---\ndate: x\n---\n\nprose"), None);
    }

    #[test]
    fn the_reasoning_is_the_text_under_the_reasoning_heading() {
        let text = good();
        assert_eq!(
            reasoning_of(&text),
            Some("Treated the long Linear block as the spine of the day.".to_string())
        );
    }

    #[test]
    fn reasoning_stops_at_the_next_heading() {
        let text = "## Reasoning\nkept the long blocks.\n\n## Key references\nnot this";
        assert_eq!(
            reasoning_of(text),
            Some("kept the long blocks.".to_string())
        );
    }

    #[test]
    fn a_summary_with_no_reasoning_section_has_no_reasoning() {
        assert_eq!(
            reasoning_of("---\ndate: x\n---\n\n## Sessions\nthings"),
            None
        );
    }
}
