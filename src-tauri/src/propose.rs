use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposeTarget {
    Rules,
    Prompt,
}

impl ProposeTarget {
    pub fn label(self) -> &'static str {
        match self {
            ProposeTarget::Rules => "rules.json",
            ProposeTarget::Prompt => "prompts/day-context.md",
        }
    }
}

/// The text the user highlighted, with enough provenance for the engine to
/// know what it is looking at and for the ledger to record where it came
/// from. `mode` is "raw" or "summary".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    pub date: NaiveDate,
    pub text: String,
    pub app: Option<String>,
    pub title: Option<String>,
    pub time_range: Option<String>,
    pub mode: String,
}

fn provenance(selection: &Selection) -> String {
    let mut out = format!("date: {}\nview: {}\n", selection.date, selection.mode);
    if let Some(app) = &selection.app {
        out.push_str(&format!("app: {app}\n"));
    }
    if let Some(title) = &selection.title {
        out.push_str(&format!("window: {title}\n"));
    }
    if let Some(range) = &selection.time_range {
        out.push_str(&format!("time: {range}\n"));
    }
    out
}

fn rules_schema() -> String {
    let locked = crate::rules::built_ins()
        .into_iter()
        .map(|b| format!("- {}: {}", b.id, b.description))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"The file is JSON of this exact shape:

{{
  "rules": [
    {{ "id": "r1", "target": {{ "app": "Slack" }}, "action": "exclude" }},
    {{ "id": "r2", "target": {{ "website": "news.ycombinator.com" }}, "action": "headings_only", "note": "too noisy" }},
    {{ "id": "r3", "target": {{ "title": "Payroll" }}, "action": "full" }}
  ]
}}

Constraints:
- `target` is exactly one of `app`, `website` or `title`. `app` and `title` match case-insensitively as substrings. `website` is a bare domain, no scheme and no path, matched against the host of the block's url reference and against the window title where no url was captured.
- `action` is exactly one of `exclude`, `headings_only`, `full`.
- `exclude` drops the window entirely. `headings_only` keeps the time, app, title and reference and drops the text. `full` is the default and exists to carve an exception out of a broader rule.
- The most specific match wins: a website rule beats a title rule beats an app rule, and within one kind the longer pattern beats the shorter.
- `id` is unique, and uses only letters, digits, hyphen and underscore. Keep existing ids as they are.
- Two rules may not share a target.
- `note` is optional, one short line saying why the rule exists.
- These protections are built in, always on, and must never appear in the file:
{locked}

Return the complete file, including every rule already in it that your change does not touch."#
    )
}

fn prompt_schema() -> String {
    format!(
        "The file is the markdown prompt handed to the engine that writes each day's summary.\n\n\
         Constraints:\n\
         - It must still ask for every one of these headings, spelled exactly: {}.\n\
         - It must not be empty.\n\
         - Keep its instructions about citing time ranges, marking inference as inference and not inventing activity. Those are what make the summaries checkable.\n\n\
         Return the complete file, not a patch and not an excerpt.",
        crate::prompt::REQUIRED_HEADINGS.join(", ")
    )
}

pub fn build_prompt(
    target: ProposeTarget,
    selection: &Selection,
    current_file: &str,
    instruction: &str,
) -> String {
    let schema = match target {
        ProposeTarget::Rules => rules_schema(),
        ProposeTarget::Prompt => prompt_schema(),
    };
    format!(
        "You are editing one configuration file for Ambient Context, a local app that keeps a \
         written record of what its user works on. You are not writing a reply and not answering \
         a question. Your entire output is the new file plus your reasoning.\n\n\
         The user highlighted this text in their own record:\n\n\
         {provenance}\n\
         ---\n{selected}\n---\n\n\
         Their instruction:\n\n{instruction}\n\n\
         The file you are editing is {label}. Here it is in full:\n\n\
         ---\n{current}\n---\n\n\
         {schema}\n\n\
         Respond with exactly two fenced blocks and nothing else that matters, in this order. \
         The fences are four backticks, because the file may contain three-backtick fences of \
         its own:\n\n\
         ````file\n(the complete new file)\n````\n\n\
         ````reasoning\n(two to four sentences: what you changed, what you left alone, and \
         anything about the instruction you were unsure of)\n````\n",
        provenance = provenance(selection),
        selected = selection.text.trim(),
        instruction = instruction.trim(),
        label = target.label(),
        current = current_file,
        schema = schema,
    )
}

const FENCE: &str = "````";

fn take_block(lines: &[&str], from: usize, tag: &str) -> Result<(String, usize), String> {
    let opener = format!("{FENCE}{tag}");
    let start = lines[from..]
        .iter()
        .position(|line| line.trim() == opener)
        .ok_or_else(|| format!("no {opener} block in the response"))?
        + from;
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.trim() == FENCE)
        .ok_or_else(|| format!("the {opener} block was never closed"))?
        + start
        + 1;
    Ok((lines[start + 1..end].join("\n"), end + 1))
}

/// Returns the new file and the model's reasoning. Prose around the blocks
/// is tolerated, because models preface things; anything else is rejected,
/// because a half-parsed configuration file is worse than a failed run.
pub fn parse_response(raw: &str) -> Result<(String, String), String> {
    let lines: Vec<&str> = raw.lines().collect();
    let (file, after_file) = take_block(&lines, 0, "file")?;
    if lines[..after_file.min(lines.len())]
        .iter()
        .any(|line| line.trim() == format!("{FENCE}reasoning"))
    {
        return Err("the reasoning block came before the file block".to_string());
    }
    let (reasoning, _) = take_block(&lines, after_file, "reasoning")?;
    if file.trim().is_empty() {
        return Err("the file block was empty".to_string());
    }
    if reasoning.trim().is_empty() {
        return Err("the reasoning block was empty".to_string());
    }
    Ok((file.trim_end().to_string(), reasoning.trim().to_string()))
}

/// A line-based diff: `  ` kept, `- ` removed, `+ ` added. Whole-file
/// context, no hunks, because both target files are short and a user
/// approving a change to their own capture rules should see all of it.
pub fn line_diff(before: &str, after: &str) -> String {
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    // table[i][j] is the LCS length of a[i..] and b[j..].
    let mut table = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            table[i][j] = if a[i] == b[j] {
                table[i + 1][j + 1] + 1
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }
    let mut out = String::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            out.push_str(&format!("  {}\n", a[i]));
            i += 1;
            j += 1;
        } else if table[i + 1][j] >= table[i][j + 1] {
            out.push_str(&format!("- {}\n", a[i]));
            i += 1;
        } else {
            out.push_str(&format!("+ {}\n", b[j]));
            j += 1;
        }
    }
    for line in &a[i..] {
        out.push_str(&format!("- {line}\n"));
    }
    for line in &b[j..] {
        out.push_str(&format!("+ {line}\n"));
    }
    out
}

/// The selection as a block ready to paste into whatever agent is already
/// open. Three backticks here, not four: this is going into someone else's
/// chat window, where three is the convention.
pub fn copy_as_context(selection: &Selection) -> String {
    let mut out = String::from("```\n");
    out.push_str(&format!(
        "From my Ambient Context record, {}",
        selection.date
    ));
    if let Some(range) = &selection.time_range {
        out.push_str(&format!(", {range}"));
    }
    out.push('\n');
    match (&selection.app, &selection.title) {
        (Some(app), Some(title)) => out.push_str(&format!("{app} \u{00b7} {title}\n")),
        (Some(app), None) => out.push_str(&format!("{app}\n")),
        _ => {}
    }
    out.push('\n');
    out.push_str(selection.text.trim());
    out.push_str("\n```\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn selection() -> Selection {
        Selection {
            date: NaiveDate::from_ymd_opt(2026, 8, 25).unwrap(),
            text: "Sponsored - You may also like".to_string(),
            app: Some("Safari".to_string()),
            title: Some("Hacker News".to_string()),
            time_range: Some("09:14\u{2013}09:41".to_string()),
            mode: "raw".to_string(),
        }
    }

    #[test]
    fn the_prompt_carries_the_selection_its_provenance_and_the_instruction() {
        let out = build_prompt(
            ProposeTarget::Rules,
            &selection(),
            "{\"rules\":[]}",
            "stop recording this site",
        );
        assert!(out.contains("Sponsored - You may also like"));
        assert!(out.contains("2026-08-25"));
        assert!(out.contains("Safari"));
        assert!(out.contains("Hacker News"));
        assert!(out.contains("09:14\u{2013}09:41"));
        assert!(out.contains("stop recording this site"));
        assert!(out.contains("{\"rules\":[]}"));
    }

    #[test]
    fn the_prompt_states_the_two_block_response_format() {
        let out = build_prompt(ProposeTarget::Rules, &selection(), "{}", "x");
        assert!(out.contains("````file"));
        assert!(out.contains("````reasoning"));
    }

    #[test]
    fn the_rules_prompt_carries_the_schema_and_the_locked_protections() {
        let out = build_prompt(ProposeTarget::Rules, &selection(), "{}", "x");
        assert!(out.contains("headings_only"));
        assert!(out.contains("builtin:"));
    }

    #[test]
    fn the_prompt_prompt_carries_the_required_headings() {
        let out = build_prompt(ProposeTarget::Prompt, &selection(), "the prompt", "be terser");
        assert!(out.contains("## Worth remembering"));
        assert!(out.contains("## Reasoning"));
    }

    #[test]
    fn parses_the_two_blocks() {
        let raw = "Here you go.\n\n````file\n{\"rules\":[]}\n````\n\n````reasoning\nNothing matched.\n````\n";
        let (file, reasoning) = parse_response(raw).unwrap();
        assert_eq!(file, "{\"rules\":[]}");
        assert_eq!(reasoning, "Nothing matched.");
    }

    #[test]
    fn a_file_block_may_contain_three_backtick_fences() {
        let raw = "````file\n# Prompt\n\n```markdown\n## Sessions\n```\n````\n\n````reasoning\nKept the template.\n````\n";
        let (file, _) = parse_response(raw).unwrap();
        assert!(file.contains("```markdown"));
        assert!(file.contains("## Sessions"));
    }

    #[test]
    fn rejects_a_response_with_no_file_block() {
        assert!(parse_response("````reasoning\nwhy\n````\n").is_err());
    }

    #[test]
    fn rejects_a_response_with_no_reasoning_block() {
        assert!(parse_response("````file\nx\n````\n").is_err());
    }

    #[test]
    fn rejects_a_reasoning_block_before_the_file_block() {
        let raw = "````reasoning\nwhy\n````\n\n````file\nx\n````\n";
        assert!(parse_response(raw).is_err());
    }

    #[test]
    fn rejects_an_unterminated_file_block() {
        assert!(parse_response("````file\nx\n").is_err());
    }

    #[test]
    fn rejects_an_empty_file_block() {
        assert!(parse_response("````file\n\n````\n\n````reasoning\nwhy\n````\n").is_err());
    }

    #[test]
    fn rejects_bare_prose() {
        assert!(parse_response("I have updated your rules for you.").is_err());
    }

    #[test]
    fn the_diff_marks_added_removed_and_kept_lines() {
        let out = line_diff("a\nb\nc\n", "a\nc\nd\n");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines, vec!["  a", "- b", "  c", "+ d"]);
    }

    #[test]
    fn an_unchanged_file_diffs_to_nothing_added_or_removed() {
        let out = line_diff("a\nb\n", "a\nb\n");
        assert!(!out.lines().any(|l| l.starts_with('+') || l.starts_with('-')));
    }

    #[test]
    fn a_diff_from_empty_is_all_additions() {
        let out = line_diff("", "a\nb\n");
        assert_eq!(out.lines().collect::<Vec<_>>(), vec!["+ a", "+ b"]);
    }

    #[test]
    fn copy_as_context_is_a_fenced_block_with_its_provenance() {
        let out = copy_as_context(&selection());
        assert!(out.starts_with("```"));
        assert!(out.trim_end().ends_with("```"));
        assert!(out.contains("2026-08-25"));
        assert!(out.contains("Safari"));
        assert!(out.contains("Hacker News"));
        assert!(out.contains("09:14\u{2013}09:41"));
        assert!(out.contains("Sponsored - You may also like"));
    }
}
