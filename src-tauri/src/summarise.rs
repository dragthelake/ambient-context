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
    /// A time range the day file cannot account for. Worse than no
    /// citation: it reads as evidence and is not.
    CitationOutsideTimeline(String),
    /// A number or hash the summary states that appears nowhere in the
    /// timeline or the knowledge base the model was given.
    UnsupportedFigure(String),
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
            Invalid::CitationOutsideTimeline(citation) => {
                write!(f, "{citation} is outside every captured block")
            }
            Invalid::UnsupportedFigure(figure) => {
                write!(
                    f,
                    "the summary states {figure}, which is nowhere in the day's evidence"
                )
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

/// Times, ranges and dates: the ISO form the prompt uses, and the written
/// forms a heading takes ("Tuesday 2 September 2026", "September 2, 2026",
/// "Sep 2026"). Blanked out of the body before the figure scan so `2026`
/// and `1100` are never read as claims about quantities. The timeline
/// carries only the ISO form, so a year written out in prose would
/// otherwise reject the whole day.
fn clock() -> &'static Regex {
    static CLOCK: OnceLock<Regex> = OnceLock::new();
    CLOCK.get_or_init(|| {
        let month = r"(?i:(?:jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)[a-z]*\.?)";
        let day = r"\d{1,2}(?:st|nd|rd|th)?";
        Regex::new(&format!(
            r"\d{{4}}-\d{{2}}-\d{{2}}|\d{{1,2}}:\d{{2}}|\b{day}\s+{month},?\s+\d{{4}}\b|\b{month}\s+{day},?\s+\d{{4}}\b|\b{month}\s+\d{{4}}\b"
        ))
        .unwrap()
    })
}

/// A quantity (three digits or more) or a hash. These are the claims a
/// model is most likely to invent, and the easiest to check.
fn figure() -> &'static Regex {
    static FIGURE: OnceLock<Regex> = OnceLock::new();
    FIGURE.get_or_init(|| Regex::new(r"\b\d{3,}\b|\b[0-9a-f]{7,40}\b").unwrap())
}

/// Whole-word containment, so `303` is not satisfied by `3030`.
fn appears_in(evidence: &str, token: &str) -> bool {
    evidence.match_indices(token).any(|(at, _)| {
        let before = evidence[..at].chars().next_back();
        let after = evidence[at + token.len()..].chars().next();
        !before.is_some_and(|c| c.is_alphanumeric()) && !after.is_some_and(|c| c.is_alphanumeric())
    })
}

/// The first figure in `body` that `evidence` cannot account for.
fn unsupported_figure(body: &str, evidence: &str) -> Option<String> {
    let masked = clock().replace_all(body, " ");
    figure()
        .find_iter(&masked)
        .map(|m| m.as_str())
        // A run of hex letters with no digit is a word ("defaced",
        // "acceded"), not a hash.
        .filter(|token| token.chars().any(|c| c.is_ascii_digit()))
        .find(|token| !appears_in(evidence, token))
        .map(|token| token.to_string())
}

pub fn validate(
    text: &str,
    max_lines: usize,
    spans: &[(u32, u32)],
    evidence: &str,
) -> Result<(), Invalid> {
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
    let body = lines[close..].join("\n");
    if !crate::cite::has_citation(&body) {
        return Err(Invalid::NoCitations);
    }
    crate::cite::citation_in_spans(&body, spans).map_err(Invalid::CitationOutsideTimeline)?;
    if let Some(figure) = unsupported_figure(&body, evidence) {
        return Err(Invalid::UnsupportedFigure(figure));
    }

    Ok(())
}

use chrono::NaiveDate;
use std::path::{Path, PathBuf};

pub fn summaries_dir(folder: &Path) -> PathBuf {
    folder.join("Summaries")
}

pub fn summary_path(folder: &Path, date: NaiveDate) -> PathBuf {
    summaries_dir(folder).join(format!("{}.md", date.format("%Y-%m-%d")))
}

pub fn build_prompt(template: &str, date: NaiveDate, timeline: &str, kb: &str) -> String {
    template
        .replace("{{DATE}}", &date.format("%Y-%m-%d").to_string())
        .replace("{{TIMELINE}}", timeline)
        .replace("{{KB}}", kb)
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

/// Every date with a Days/ folder. Flat 0.1 day files are ignored.
pub fn list_captured(folder: &Path) -> Vec<NaiveDate> {
    let Ok(entries) = std::fs::read_dir(crate::writer::days_dir(folder)) else {
        return Vec::new();
    };
    let mut dates: Vec<NaiveDate> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            NaiveDate::parse_from_str(&entry.file_name().to_string_lossy(), "%Y-%m-%d").ok()
        })
        .collect();
    dates.sort();
    dates
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

    /// The fixture day: one block from 09:00 to 11:00.
    fn spans() -> Vec<(u32, u32)> {
        vec![(540, 660)]
    }

    const EVIDENCE: &str = "## 09:00\u{2013}11:00 \u{b7} Zed \u{b7} jobs.rs\n\nran 303 tests\n";

    fn check(text: &str) -> Result<(), Invalid> {
        validate(text, MAX_SUMMARY_LINES, &spans(), EVIDENCE)
    }

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
        assert!(check(&good()).is_ok());
    }

    #[test]
    fn rejects_empty_output() {
        assert!(matches!(check("   \n  "), Err(Invalid::Empty)));
    }

    #[test]
    fn rejects_output_with_no_frontmatter() {
        let text = "# A day\n\nSome prose.\n\n## Sessions\n09:00-11:00 things";
        assert!(matches!(check(text), Err(Invalid::NoFrontmatter)));
    }

    #[test]
    fn rejects_frontmatter_missing_the_type_field() {
        let text = good().replace("type: day-context\n", "");
        assert!(matches!(check(&text), Err(Invalid::MissingField("type"))));
    }

    #[test]
    fn rejects_frontmatter_missing_the_date_field() {
        let text = good().replace("date: 2026-08-28\n", "");
        assert!(matches!(check(&text), Err(Invalid::MissingField("date"))));
    }

    #[test]
    fn rejects_output_with_no_sections() {
        let text = "---\ndate: 2026-08-28\ntype: day-context\n---\n\nJust prose, no headings.";
        assert!(matches!(check(text), Err(Invalid::NoSections)));
    }

    #[test]
    fn rejects_a_summary_that_never_states_its_reasoning() {
        let text = good().replace("## Reasoning", "## Notes");
        assert!(matches!(
            check(&text),
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
        assert!(matches!(check(&text), Err(Invalid::NoCitations)));
    }

    #[test]
    fn accepts_ranges_written_with_either_dash() {
        for sep in ["-", "\u{2013}"] {
            let text = good().replace("09:00-11:00", &format!("09:00{sep}11:00"));
            assert!(check(&text).is_ok(), "failed on {sep:?}");
        }
    }

    #[test]
    fn a_citation_the_day_never_recorded_is_rejected_and_named() {
        let text = good().replace("09:00-11:00", "14:00-14:30");
        assert_eq!(
            check(&text),
            Err(Invalid::CitationOutsideTimeline("14:00-14:30".to_string()))
        );
    }

    #[test]
    fn a_figure_the_evidence_does_not_carry_is_rejected_and_named() {
        let text = good().replace("the thing.", "the thing, 512 tests green.");
        assert_eq!(
            check(&text),
            Err(Invalid::UnsupportedFigure("512".to_string()))
        );
    }

    #[test]
    fn a_word_spelt_in_hex_letters_is_not_a_hash() {
        let text = good().replace(
            "the thing.",
            "the thing; the page was defaced and they acceded.",
        );
        assert!(check(&text).is_ok());
    }

    #[test]
    fn a_figure_the_evidence_carries_is_accepted() {
        let text = good().replace("the thing.", "the thing, 303 tests green.");
        assert_eq!(check(&text), Ok(()));
    }

    #[test]
    fn a_commit_hash_the_evidence_does_not_carry_is_rejected() {
        let text = good().replace("building the thing.", "landed dead6ee on main.");
        assert_eq!(
            check(&text),
            Err(Invalid::UnsupportedFigure("dead6ee".to_string()))
        );
    }

    #[test]
    fn times_and_dates_are_not_figures() {
        // `09:00`, the `1100` inside a range and the summary's own date
        // would all be unsupported figures without the mask.
        let text = good().replace("the thing.", "the thing, carried over from 2026-08-27.");
        assert_eq!(check(&text), Ok(()));
    }

    #[test]
    fn dates_written_out_are_not_figures() {
        // A heading or sentence that writes the year in prose is a date,
        // not a count, and the evidence only ever carries the ISO form.
        for phrase in [
            "carried over from 2 September 2026.",
            "carried over from Tuesday 2nd September 2026.",
            "carried over from September 2, 2026.",
            "carried over from Sept 2026.",
        ] {
            let text = good().replace("the thing.", &format!("the thing, {phrase}"));
            assert_eq!(check(&text), Ok(()), "{phrase}");
        }
        // A bare year-sized number is still a figure.
        let text = good().replace("the thing.", "the thing, 1999 tests green.");
        assert_eq!(
            check(&text),
            Err(Invalid::UnsupportedFigure("1999".to_string()))
        );
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
        assert!(matches!(check(&text), Err(Invalid::NoCitations)));
    }

    #[test]
    fn rejects_output_over_the_line_budget() {
        let mut text = good();
        for i in 0..50 {
            text.push_str(&format!("\nline {i}"));
        }
        assert!(matches!(
            validate(&text, 20, &spans(), EVIDENCE),
            Err(Invalid::TooLong { max: 20, .. })
        ));
    }

    #[test]
    fn a_model_that_wraps_output_in_a_code_fence_still_passes() {
        // Every CLI does this at least sometimes. Rejecting it would fail
        // for a formatting habit rather than a content problem.
        let text = format!("```markdown\n{}\n```", good());
        assert!(check(&text).is_ok());
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
    fn the_prompt_carries_the_date_timeline_and_knowledge_base() {
        let out = build_prompt(
            "Date: {{DATE}}\nTimeline:\n{{TIMELINE}}\nKB:\n{{KB}}",
            date(2026, 8, 28),
            "## 09:00 block",
            "people.md",
        );
        assert_eq!(
            out,
            "Date: 2026-08-28\nTimeline:\n## 09:00 block\nKB:\npeople.md"
        );
    }

    #[test]
    fn day_file_is_no_longer_a_placeholder() {
        let out = build_prompt("Body: {{DAY_FILE}}", date(2026, 8, 28), "timeline", "kb");
        assert_eq!(out, "Body: {{DAY_FILE}}");
    }

    #[test]
    fn the_bundled_summary_prompt_carries_its_placeholders_and_reasoning() {
        let bundled = crate::prompt::PromptId::DayContext.bundled();
        assert!(bundled.contains("{{DATE}}"));
        assert!(bundled.contains("{{TIMELINE}}"));
        assert!(bundled.contains("{{KB}}"));
        assert!(bundled.contains("## Reasoning"));
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
        for date in [date(2026, 8, 27), date(2026, 8, 28)] {
            let path = crate::writer::DayFile::Apps.path(dir.path(), date);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "x").unwrap();
        }
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
