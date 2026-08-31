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

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Proposal {
    pub id: String,
    pub target: ProposeTarget,
    pub before: String,
    pub after: String,
    pub diff: String,
    pub reasoning: String,
    pub ledger_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProposeError {
    NoEngine,
    EngineFailed { stderr: String },
    Invalid { reason: String, raw: String },
}

impl std::fmt::Display for ProposeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProposeError::NoEngine => write!(
                f,
                "no engine is connected. Connect one in Settings to use this."
            ),
            ProposeError::EngineFailed { stderr } => write!(f, "the engine failed: {stderr}"),
            ProposeError::Invalid { reason, .. } => {
                write!(f, "the engine's answer was not usable: {reason}")
            }
        }
    }
}

fn read_target(config_dir: &Path, target: ProposeTarget) -> String {
    match target {
        ProposeTarget::Rules => serde_json::to_string_pretty(&crate::rules::load(config_dir))
            .unwrap_or_else(|_| "{\n  \"rules\": []\n}".to_string()),
        ProposeTarget::Prompt => crate::prompt::current(config_dir),
    }
}

/// The validation `apply` repeats, so a proposal can never be written by a
/// path that did not check it.
fn check(target: ProposeTarget, file: &str) -> Result<(), String> {
    match target {
        ProposeTarget::Rules => {
            crate::rules::parse(file).map(|_| ()).map_err(|e| e.to_string())
        }
        ProposeTarget::Prompt => crate::prompt::validate(file).map_err(|e| e.to_string()),
    }
}

fn action_name(target: ProposeTarget) -> &'static str {
    match target {
        ProposeTarget::Rules => "propose_rules",
        ProposeTarget::Prompt => "propose_prompt",
    }
}

fn target_path(config_dir: &Path, target: ProposeTarget) -> PathBuf {
    match target {
        ProposeTarget::Rules => crate::rules::rules_path(config_dir),
        ProposeTarget::Prompt => crate::prompt::prompt_path(config_dir),
    }
}

fn inputs_for(config_dir: &Path, target: ProposeTarget) -> Vec<crate::ledger::Input> {
    crate::ledger::hash_file(&target_path(config_dir, target))
        .map(|input| vec![input])
        .unwrap_or_default()
}

pub fn propose(
    config_dir: &Path,
    folder: &Path,
    engine: &crate::engine::Engine,
    target: ProposeTarget,
    selection: Selection,
    instruction: &str,
) -> Result<Proposal, ProposeError> {
    let before = read_target(config_dir, target);
    let base = build_prompt(target, &selection, &before, instruction);
    let inputs = inputs_for(config_dir, target);

    let mut prompt = base.clone();
    let mut last_reason = String::new();
    let mut last_raw = String::new();

    // Two attempts, never three. The second restates the exact failure,
    // which is the cheapest thing that turns a near-miss into a usable
    // answer and the only mitigation this feature gets.
    for attempt in 0..2 {
        let run = crate::engine::run(engine, &prompt);
        if run.timed_out || run.status != 0 {
            let stderr = if run.timed_out {
                format!("timed out after {}s", engine.timeout_secs)
            } else {
                run.stderr.clone()
            };
            let _ = ledger(
                folder,
                target,
                engine,
                &prompt,
                inputs.clone(),
                Some(run.stdout.clone()),
                None,
                crate::ledger::Disposition::Failed {
                    stderr: stderr.clone(),
                },
            );
            return Err(ProposeError::EngineFailed { stderr });
        }
        last_raw = run.stdout.clone();
        match parse_response(&run.stdout).and_then(|(file, reasoning)| {
            check(target, &file)?;
            Ok((file, reasoning))
        }) {
            Ok((after, reasoning)) => {
                let ledger_path = ledger(
                    folder,
                    target,
                    engine,
                    &prompt,
                    inputs,
                    Some(run.stdout.clone()),
                    Some(reasoning.clone()),
                    crate::ledger::Disposition::Accepted,
                )
                .unwrap_or_default();
                return Ok(Proposal {
                    id: format!("p-{}", chrono::Local::now().timestamp_micros()),
                    target,
                    diff: line_diff(&before, &after),
                    before,
                    after,
                    reasoning,
                    ledger_path,
                });
            }
            Err(reason) => {
                last_reason = reason.clone();
                if attempt == 0 {
                    prompt = format!(
                        "{base}\n\nYour previous answer could not be used: {reason}\nAnswer again, \
                         following the response format exactly.\n"
                    );
                }
            }
        }
    }

    let _ = ledger(
        folder,
        target,
        engine,
        &prompt,
        inputs,
        Some(last_raw.clone()),
        None,
        crate::ledger::Disposition::Rejected {
            reason: last_reason.clone(),
        },
    );
    Err(ProposeError::Invalid {
        reason: last_reason,
        raw: last_raw,
    })
}

#[allow(clippy::too_many_arguments)]
fn ledger(
    folder: &Path,
    target: ProposeTarget,
    engine: &crate::engine::Engine,
    prompt: &str,
    inputs: Vec<crate::ledger::Input>,
    output: Option<String>,
    reasoning: Option<String>,
    disposition: crate::ledger::Disposition,
) -> std::io::Result<PathBuf> {
    crate::ledger::append(
        folder,
        &crate::ledger::Entry {
            at: chrono::Local::now(),
            trigger: crate::ledger::Trigger::Propose,
            action: action_name(target).to_string(),
            prompt_id: Some(action_name(target).to_string()),
            prompt_sha256: Some(crate::ledger::sha256_of(prompt.as_bytes())),
            engine: Some(engine.label.clone()),
            inputs,
            output,
            reasoning,
            disposition,
        },
    )
}

/// Validates again before writing, because a proposal is data that crossed
/// the process boundary into the webview and came back.
pub fn apply(config_dir: &Path, folder: &Path, proposal: &Proposal) -> Result<(), String> {
    check(proposal.target, &proposal.after)?;
    match proposal.target {
        ProposeTarget::Rules => {
            let parsed = crate::rules::parse(&proposal.after).map_err(|e| e.to_string())?;
            crate::rules::save(config_dir, &parsed).map_err(|e| e.to_string())?;
        }
        ProposeTarget::Prompt => {
            crate::prompt::set(config_dir, &proposal.after).map_err(|e| e.to_string())?;
        }
    }
    let _ = crate::ledger::append(
        folder,
        &crate::ledger::Entry {
            at: chrono::Local::now(),
            trigger: crate::ledger::Trigger::Propose,
            action: format!("apply_{}", action_name(proposal.target)),
            prompt_id: None,
            prompt_sha256: None,
            engine: None,
            inputs: inputs_for(config_dir, proposal.target),
            output: Some(proposal.after.clone()),
            reasoning: Some(proposal.reasoning.clone()),
            disposition: crate::ledger::Disposition::Applied,
        },
    );
    Ok(())
}

pub fn discard(folder: &Path, proposal: &Proposal) -> std::io::Result<()> {
    crate::ledger::append(
        folder,
        &crate::ledger::Entry {
            at: chrono::Local::now(),
            trigger: crate::ledger::Trigger::Propose,
            action: format!("discard_{}", action_name(proposal.target)),
            prompt_id: None,
            prompt_sha256: None,
            engine: None,
            inputs: Vec::new(),
            output: Some(proposal.after.clone()),
            reasoning: Some(proposal.reasoning.clone()),
            disposition: crate::ledger::Disposition::Discarded,
        },
    )
    .map(|_| ())
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

    use tempfile::tempdir;

    fn fake_engine(script: &str) -> crate::engine::Engine {
        crate::engine::Engine {
            label: "fake".to_string(),
            command: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), script.to_string()],
            timeout_secs: 10,
        }
    }

    const GOOD_RULES: &str = "cat >/dev/null; printf '%s\\n' '````file' '{\"rules\":[{\"id\":\"r1\",\"target\":{\"app\":\"Slack\"},\"action\":\"exclude\"}]}' '````' '' '````reasoning' 'Excluded Slack.' '````'";

    #[test]
    fn a_valid_rules_proposal_returns_a_diff_and_writes_nothing() {
        let config = tempdir().unwrap();
        let folder = tempdir().unwrap();
        let proposal = propose(
            config.path(),
            folder.path(),
            &fake_engine(GOOD_RULES),
            ProposeTarget::Rules,
            selection(),
            "never record Slack",
        )
        .unwrap();
        assert!(proposal.after.contains("\"exclude\""));
        assert!(proposal.diff.contains('+'));
        assert_eq!(proposal.reasoning, "Excluded Slack.");
        assert!(crate::rules::load(config.path()).rules.is_empty());
    }

    #[test]
    fn a_valid_proposal_ledgers_accepted() {
        let config = tempdir().unwrap();
        let folder = tempdir().unwrap();
        let proposal = propose(
            config.path(),
            folder.path(),
            &fake_engine(GOOD_RULES),
            ProposeTarget::Rules,
            selection(),
            "never record Slack",
        )
        .unwrap();
        let written = std::fs::read_to_string(&proposal.ledger_path).unwrap();
        assert!(written.contains("propose_rules"));
        assert!(written.contains("accepted"));
    }

    #[test]
    fn a_failing_engine_is_a_failure_and_is_ledgered() {
        let config = tempdir().unwrap();
        let folder = tempdir().unwrap();
        let err = propose(
            config.path(),
            folder.path(),
            &fake_engine("cat >/dev/null; echo 'not logged in' >&2; exit 1"),
            ProposeTarget::Rules,
            selection(),
            "x",
        )
        .unwrap_err();
        assert!(matches!(err, ProposeError::EngineFailed { .. }));
        assert!(days_ledger(folder.path()).contains("failed:"));
    }

    #[test]
    fn an_invalid_response_is_retried_once_and_then_rejected() {
        let config = tempdir().unwrap();
        let folder = tempdir().unwrap();
        // Counts its own invocations through a file in the capture folder.
        let counter = folder.path().join("runs");
        let script = format!(
            "cat >/dev/null; echo x >> {}; echo 'here are your rules'",
            counter.display()
        );
        let err = propose(
            config.path(),
            folder.path(),
            &fake_engine(&script),
            ProposeTarget::Rules,
            selection(),
            "x",
        )
        .unwrap_err();
        assert!(matches!(err, ProposeError::Invalid { .. }));
        assert_eq!(
            std::fs::read_to_string(&counter).unwrap().lines().count(),
            2
        );
        assert!(days_ledger(folder.path()).contains("rejected:"));
    }

    #[test]
    fn a_response_that_breaks_the_rules_schema_is_rejected() {
        let config = tempdir().unwrap();
        let folder = tempdir().unwrap();
        let script = "cat >/dev/null; printf '%s\\n' '````file' '{\"rules\":[{\"id\":\"r1\",\"target\":{\"app\":\"1Password\"},\"action\":\"full\"}]}' '````' '' '````reasoning' 'why' '````'";
        let err = propose(
            config.path(),
            folder.path(),
            &fake_engine(script),
            ProposeTarget::Rules,
            selection(),
            "x",
        )
        .unwrap_err();
        assert!(matches!(err, ProposeError::Invalid { .. }));
    }

    #[test]
    fn apply_writes_the_file_and_ledgers_applied() {
        let config = tempdir().unwrap();
        let folder = tempdir().unwrap();
        let proposal = propose(
            config.path(),
            folder.path(),
            &fake_engine(GOOD_RULES),
            ProposeTarget::Rules,
            selection(),
            "never record Slack",
        )
        .unwrap();
        apply(config.path(), folder.path(), &proposal).unwrap();
        let saved = crate::rules::load(config.path());
        assert_eq!(saved.rules.len(), 1);
        assert_eq!(saved.rules[0].action, crate::rules::Action::Exclude);
        assert!(days_ledger(folder.path()).contains("applied"));
    }

    #[test]
    fn apply_revalidates_and_refuses_a_tampered_proposal() {
        let config = tempdir().unwrap();
        let folder = tempdir().unwrap();
        let mut proposal = propose(
            config.path(),
            folder.path(),
            &fake_engine(GOOD_RULES),
            ProposeTarget::Rules,
            selection(),
            "never record Slack",
        )
        .unwrap();
        proposal.after = "{ not json".to_string();
        assert!(apply(config.path(), folder.path(), &proposal).is_err());
        assert!(crate::rules::load(config.path()).rules.is_empty());
    }

    #[test]
    fn discard_writes_nothing_and_ledgers_discarded() {
        let config = tempdir().unwrap();
        let folder = tempdir().unwrap();
        let proposal = propose(
            config.path(),
            folder.path(),
            &fake_engine(GOOD_RULES),
            ProposeTarget::Rules,
            selection(),
            "never record Slack",
        )
        .unwrap();
        discard(folder.path(), &proposal).unwrap();
        assert!(crate::rules::load(config.path()).rules.is_empty());
        assert!(days_ledger(folder.path()).contains("discarded"));
    }

    fn days_ledger(folder: &std::path::Path) -> String {
        let dir = folder.join("Ledger");
        std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n")
    }
}
