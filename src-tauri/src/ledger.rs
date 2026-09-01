use chrono::{DateTime, Local, NaiveDate};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// What started the action. Every surface that can move a model is here, so
/// a reader can tell an agent's change from a person's.
#[derive(Debug, Clone, PartialEq)]
pub enum Trigger {
    Schedule,
    OnDemand,
    Propose,
    Mcp { client: String },
    Settings,
}

/// How it ended. Failures are the entries that earn this module.
#[derive(Debug, Clone, PartialEq)]
pub enum Disposition {
    Accepted,
    Rejected { reason: String },
    Applied,
    Discarded,
    Failed { stderr: String },
}

/// A file that went into the brief, by path and content hash. Never a copy:
/// the ledger must not become a second store of captured text.
#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    pub path: PathBuf,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub at: DateTime<Local>,
    pub trigger: Trigger,
    /// "summarise_day", "propose_rules", "set_config", and so on.
    pub action: String,
    pub prompt_id: Option<String>,
    /// A prompt changed by an app update silently rewrites history without
    /// this.
    pub prompt_sha256: Option<String>,
    /// Still "engine" though the rest of the app says agent. This is written
    /// into entries in the user's capture folder, and a query over their
    /// ledger should not have to know which app version wrote each line.
    pub engine: Option<String>,
    pub inputs: Vec<Input>,
    /// What the agent produced, verbatim.
    pub output: Option<String>,
    /// The model's stated account of its own choices.
    pub reasoning: Option<String>,
    pub disposition: Disposition,
}

pub fn sha256_of(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn hash_file(path: &Path) -> std::io::Result<Input> {
    let bytes = fs::read(path)?;
    Ok(Input {
        path: path.to_path_buf(),
        sha256: sha256_of(&bytes),
    })
}

pub fn ledger_dir(folder: &Path) -> PathBuf {
    folder.join("Ledger")
}

pub fn ledger_path(folder: &Path, date: NaiveDate) -> PathBuf {
    ledger_dir(folder).join(format!("{}.md", date.format("%Y-%m-%d")))
}

fn trigger_label(trigger: &Trigger) -> String {
    match trigger {
        Trigger::Schedule => "schedule".to_string(),
        Trigger::OnDemand => "on demand".to_string(),
        Trigger::Propose => "highlight to instruct".to_string(),
        Trigger::Mcp { client } => format!("mcp: {client}"),
        Trigger::Settings => "settings".to_string(),
    }
}

fn disposition_label(disposition: &Disposition) -> String {
    match disposition {
        Disposition::Accepted => "accepted".to_string(),
        Disposition::Rejected { reason } => format!("rejected: {reason}"),
        Disposition::Applied => "applied".to_string(),
        Disposition::Discarded => "discarded".to_string(),
        Disposition::Failed { stderr } => format!("failed: {}", stderr.trim()),
    }
}

/// One markdown section per entry. Six backticks around the output because
/// model output routinely contains triple-backtick fences of its own, and a
/// ledger that swallows its own formatting is not readable without the app.
pub fn render(entry: &Entry) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "\n## {} \u{00b7} {}\n\n",
        entry.at.format("%H:%M:%S"),
        entry.action
    ));
    out.push_str(&format!("- trigger: {}\n", trigger_label(&entry.trigger)));
    if let Some(id) = &entry.prompt_id {
        let hash = entry.prompt_sha256.as_deref().unwrap_or("unhashed");
        out.push_str(&format!("- prompt: {id} sha256 {hash}\n"));
    }
    if let Some(engine) = &entry.engine {
        out.push_str(&format!("- engine: {engine}\n"));
    }
    for input in &entry.inputs {
        out.push_str(&format!(
            "- input: {} sha256 {}\n",
            input.path.display(),
            input.sha256
        ));
    }
    out.push_str(&format!(
        "- disposition: {}\n",
        disposition_label(&entry.disposition)
    ));
    if let Some(reasoning) = &entry.reasoning {
        out.push_str(&format!("\n### Reasoning\n\n{}\n", reasoning.trim()));
    }
    if let Some(output) = &entry.output {
        out.push_str(&format!(
            "\n### Output\n\n``````\n{}\n``````\n",
            output.trim_end()
        ));
    }
    out
}

/// Appends one entry to the ledger for the day the action happened.
/// Append-only by construction: there is no function here that rewrites an
/// existing entry.
pub fn append(folder: &Path, entry: &Entry) -> std::io::Result<PathBuf> {
    let dir = ledger_dir(folder);
    fs::create_dir_all(&dir)?;
    let date = entry.at.date_naive();
    let path = ledger_path(folder, date);
    let is_new = !path.exists();

    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    if is_new {
        file.write_all(
            format!(
                "---\ndate: {}\ntype: ledger\n---\n",
                date.format("%Y-%m-%d")
            )
            .as_bytes(),
        )?;
    }
    file.write_all(render(entry).as_bytes())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, TimeZone};
    use tempfile::tempdir;

    fn entry() -> Entry {
        Entry {
            at: Local.with_ymd_and_hms(2026, 8, 31, 6, 2, 11).unwrap(),
            trigger: Trigger::Schedule,
            action: "summarise_day".to_string(),
            prompt_id: Some("day-context".to_string()),
            prompt_sha256: Some(sha256_of(b"the prompt")),
            engine: Some("Claude Code".to_string()),
            inputs: vec![Input {
                path: std::path::PathBuf::from("/tmp/ac/2026-08-30.md"),
                sha256: sha256_of(b"the day"),
            }],
            output: Some("---\ndate: 2026-08-30\n---\n\n# A day".to_string()),
            reasoning: Some("Kept the long blocks.".to_string()),
            disposition: Disposition::Accepted,
        }
    }

    #[test]
    fn the_empty_digest_is_the_published_sha256_of_no_bytes() {
        assert_eq!(
            sha256_of(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn the_same_bytes_hash_the_same_and_different_bytes_do_not() {
        assert_eq!(sha256_of(b"a day"), sha256_of(b"a day"));
        assert_ne!(sha256_of(b"a day"), sha256_of(b"a day "));
    }

    #[test]
    fn hashing_a_file_carries_its_path_and_the_digest_of_its_bytes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("2026-08-30.md");
        std::fs::write(&path, b"the day").unwrap();
        let input = hash_file(&path).unwrap();
        assert_eq!(input.path, path);
        assert_eq!(input.sha256, sha256_of(b"the day"));
    }

    #[test]
    fn an_entry_renders_every_field_it_carries() {
        let text = render(&entry());
        assert!(text.contains("summarise_day"), "action");
        assert!(text.contains("schedule"), "trigger");
        assert!(text.contains("day-context"), "prompt id");
        assert!(text.contains("Claude Code"), "engine");
        assert!(text.contains("/tmp/ac/2026-08-30.md"), "input path");
        assert!(text.contains(&sha256_of(b"the day")), "input hash");
        assert!(text.contains("# A day"), "output");
        assert!(text.contains("Kept the long blocks."), "reasoning");
        assert!(text.contains("accepted"), "disposition");
    }

    #[test]
    fn a_rejection_renders_its_reason_and_a_failure_renders_its_stderr() {
        let mut rejected = entry();
        rejected.disposition = Disposition::Rejected {
            reason: "the summary has no frontmatter block".to_string(),
        };
        assert!(render(&rejected).contains("no frontmatter block"));

        let mut failed = entry();
        failed.disposition = Disposition::Failed {
            stderr: "not logged in".to_string(),
        };
        assert!(render(&failed).contains("not logged in"));
    }

    #[test]
    fn appending_creates_a_file_named_for_the_day_the_action_happened() {
        let dir = tempdir().unwrap();
        let path = append(dir.path(), &entry()).unwrap();
        assert_eq!(path, dir.path().join("Ledger").join("2026-08-31.md"));
        assert!(path.is_file());
    }

    #[test]
    fn appending_twice_keeps_both_entries() {
        let dir = tempdir().unwrap();
        append(dir.path(), &entry()).unwrap();
        let mut second = entry();
        second.action = "regenerate_day".to_string();
        let path = append(dir.path(), &second).unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches("\n## ").count(), 2);
        assert!(text.contains("summarise_day"));
        assert!(text.contains("regenerate_day"));
    }

    #[test]
    fn an_mcp_trigger_names_the_client_that_asked() {
        let mut by_agent = entry();
        by_agent.trigger = Trigger::Mcp {
            client: "Claude Code".to_string(),
        };
        assert!(render(&by_agent).contains("mcp: Claude Code"));
    }
}
