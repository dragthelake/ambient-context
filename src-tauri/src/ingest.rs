use crate::prompt::PromptId;
use crate::writer::DayFile;
use chrono::NaiveDate;
use regex::Regex;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const MAX_KB_LINES: usize = 200;
pub const KB_FILES: [&str; 6] = [
    "people.md",
    "commitments.md",
    "threads.md",
    "products.md",
    "issues.md",
    "reading.md",
];
const NOTHING: &str = "Nothing evident.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Call {
    Messages,
    Apps,
    Websites,
}

impl Call {
    pub const ALL: [Call; 3] = [Call::Messages, Call::Apps, Call::Websites];

    pub fn action(self) -> &'static str {
        match self {
            Call::Messages => "ingest_messages",
            Call::Apps => "ingest_apps",
            Call::Websites => "ingest_websites",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Call::Messages => "messages",
            Call::Apps => "apps",
            Call::Websites => "websites",
        }
    }

    pub fn prompt(self) -> PromptId {
        match self {
            Call::Messages => PromptId::IngestMessages,
            Call::Apps => PromptId::IngestApps,
            Call::Websites => PromptId::IngestWebsites,
        }
    }

    pub fn source(self) -> DayFile {
        match self {
            Call::Messages => DayFile::Messages,
            Call::Apps => DayFile::Apps,
            Call::Websites => DayFile::Websites,
        }
    }

    pub fn files(self) -> &'static [&'static str] {
        match self {
            Call::Messages => &["people.md", "commitments.md"],
            Call::Apps => &["threads.md", "products.md", "issues.md"],
            Call::Websites => &["reading.md"],
        }
    }
}

pub fn kb_root(folder: &Path) -> PathBuf {
    folder.join("KB")
}

pub fn kb_dir(folder: &Path, date: NaiveDate) -> PathBuf {
    kb_root(folder).join(date.format("%Y-%m-%d").to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    pub files: Vec<(String, String)>,
    pub reasoning: Option<String>,
}

pub fn split_output(text: &str) -> Split {
    let body = crate::summarise::unfence(text);
    let mut files: Vec<(String, String)> = Vec::new();
    let mut reasoning: Option<String> = None;
    let mut current: Option<(String, String)> = None;
    let mut in_reasoning = false;

    let flush = |current: &mut Option<(String, String)>, files: &mut Vec<(String, String)>| {
        if let Some((name, body)) = current.take() {
            files.push((name, body.trim().to_string()));
        }
    };

    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("<<<file:") {
            flush(&mut current, &mut files);
            in_reasoning = false;
            let name = rest.trim_end_matches(">>>").trim().to_string();
            current = Some((name, String::new()));
            continue;
        }
        if trimmed == "<<<reasoning>>>" {
            flush(&mut current, &mut files);
            in_reasoning = true;
            reasoning = Some(String::new());
            continue;
        }
        if in_reasoning {
            if let Some(r) = reasoning.as_mut() {
                r.push_str(line);
                r.push('\n');
            }
        } else if let Some((_, body)) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    flush(&mut current, &mut files);
    Split {
        files,
        reasoning: reasoning
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invalid {
    Empty,
    MissingFile(String),
    DuplicateFile(String),
    UnexpectedFile(String),
    NoCitation {
        file: String,
        line: String,
    },
    CitationOutsideTimeline {
        file: String,
        citation: String,
    },
    TooLong {
        file: String,
        lines: usize,
        max: usize,
    },
}

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Invalid::Empty => write!(f, "the agent returned nothing"),
            Invalid::MissingFile(name) => {
                write!(f, "the output has no <<<file: {name}>>> section")
            }
            Invalid::DuplicateFile(name) => {
                write!(f, "the output has two <<<file: {name}>>> sections")
            }
            Invalid::UnexpectedFile(name) => write!(
                f,
                "the output has a <<<file: {name}>>> section this call does not write"
            ),
            Invalid::NoCitation { file, line } => {
                write!(f, "{file}: a line carries no time range: {line:?}")
            }
            Invalid::CitationOutsideTimeline { file, citation } => {
                write!(f, "{file}: {citation} is outside every captured block")
            }
            Invalid::TooLong { file, lines, max } => {
                write!(f, "{file} is {lines} lines, over the {max} line budget")
            }
        }
    }
}

fn citation() -> &'static Regex {
    static CITATION: OnceLock<Regex> = OnceLock::new();
    CITATION.get_or_init(|| Regex::new(r"\b(\d{2}):(\d{2})[-\x{2013}](\d{2}):(\d{2})\b").unwrap())
}

fn inside(minute: u32, spans: &[(u32, u32)]) -> bool {
    spans.iter().any(|(s, e)| minute >= *s && minute <= *e)
        || spans
            .iter()
            .any(|(s, e)| minute + 24 * 60 >= *s && minute + 24 * 60 <= *e)
}

pub fn validate(call: Call, split: &Split, spans: &[(u32, u32)]) -> Result<(), Invalid> {
    if split.files.is_empty() {
        return Err(Invalid::Empty);
    }
    for expected in call.files() {
        if !split.files.iter().any(|(n, _)| n == expected) {
            return Err(Invalid::MissingFile((*expected).to_string()));
        }
    }
    for (name, _) in &split.files {
        if !call.files().contains(&name.as_str()) {
            return Err(Invalid::UnexpectedFile(name.clone()));
        }
        if split.files.iter().filter(|(n, _)| n == name).count() > 1 {
            return Err(Invalid::DuplicateFile(name.clone()));
        }
    }
    for expected in call.files() {
        let Some((_, body)) = split.files.iter().find(|(n, _)| n == expected) else {
            continue;
        };
        let lines: Vec<&str> = body.lines().collect();
        if lines.len() > MAX_KB_LINES {
            return Err(Invalid::TooLong {
                file: (*expected).to_string(),
                lines: lines.len(),
                max: MAX_KB_LINES,
            });
        }
        if body.trim() == NOTHING {
            continue;
        }
        for line in lines {
            let t = line.trim();
            if t.is_empty() || t.starts_with("## ") || t == NOTHING {
                continue;
            }
            let Some(caps) = citation().captures(t) else {
                return Err(Invalid::NoCitation {
                    file: (*expected).to_string(),
                    line: t.chars().take(80).collect(),
                });
            };
            let n = |i: usize| caps[i].parse::<u32>().unwrap_or(0);
            let (start, end) = (n(1) * 60 + n(2), n(3) * 60 + n(4));
            if !inside(start, spans) || !inside(end, spans) {
                return Err(Invalid::CitationOutsideTimeline {
                    file: (*expected).to_string(),
                    citation: caps[0].to_string(),
                });
            }
        }
    }
    Ok(())
}

/// Over `max_chars`, the longest block bodies are dropped first, headings
/// and reference lines kept. Returns the text and how many blocks lost
/// their body.
pub fn trim_input(text: &str, max_chars: usize) -> (String, usize) {
    if text.chars().count() <= max_chars {
        return (text.to_string(), 0);
    }
    let mut blocks: Vec<(Vec<String>, Vec<String>)> = Vec::new();
    let mut preamble: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.starts_with("## ") {
            blocks.push((vec![line.to_string()], Vec::new()));
            continue;
        }
        match blocks.last_mut() {
            None => preamble.push(line.to_string()),
            Some((head, body)) => {
                if body.is_empty()
                    && (line.is_empty()
                        || line.starts_with("file: ")
                        || line.starts_with("url: ")
                        || line.starts_with("routed: "))
                {
                    head.push(line.to_string());
                } else {
                    body.push(line.to_string());
                }
            }
        }
    }
    let mut trimmed = 0usize;
    let total = |blocks: &[(Vec<String>, Vec<String>)]| -> usize {
        preamble
            .iter()
            .map(|l| l.chars().count() + 1)
            .sum::<usize>()
            + blocks
                .iter()
                .map(|(h, b)| {
                    h.iter()
                        .chain(b.iter())
                        .map(|l| l.chars().count() + 1)
                        .sum::<usize>()
                })
                .sum::<usize>()
    };
    while total(&blocks) > max_chars {
        let Some((index, _)) = blocks
            .iter()
            .enumerate()
            .filter(|(_, (_, b))| b.len() > 1 || (b.len() == 1 && !b[0].starts_with("[trimmed")))
            .max_by_key(|(_, (_, b))| b.iter().map(|l| l.chars().count()).sum::<usize>())
        else {
            break;
        };
        let n = blocks[index].1.len();
        blocks[index].1 = vec![format!("[trimmed {n} lines]")];
        trimmed += 1;
    }
    let mut out = preamble.join("\n");
    if !preamble.is_empty() {
        out.push('\n');
    }
    for (head, body) in blocks {
        for line in head.iter().chain(body.iter()) {
            out.push_str(line);
            out.push('\n');
        }
        if !body.is_empty() && !body.last().unwrap().is_empty() {
            out.push('\n');
        }
    }
    (out, trimmed)
}

pub struct Frontmatter {
    pub date: NaiveDate,
    pub source: String,
    pub generated_by: String,
    pub prompt_sha256: String,
}

fn render_kb_file(fm: &Frontmatter, body: &str) -> String {
    format!(
        "---\ndate: {}\nkind: kb\nsource: {}\ngenerated_by: {}\nprompt_sha256: {}\n---\n\n{}\n",
        fm.date.format("%Y-%m-%d"),
        fm.source,
        fm.generated_by,
        fm.prompt_sha256,
        body.trim()
    )
}

pub fn write_call(
    folder: &Path,
    date: NaiveDate,
    call: Call,
    files: &[(String, String)],
    fm: &Frontmatter,
) -> std::io::Result<()> {
    let tmp = kb_root(folder).join(format!(".tmp-{}-{}", date.format("%Y-%m-%d"), call.label()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)?;
    for (name, body) in files {
        std::fs::write(tmp.join(name), render_kb_file(fm, body))?;
    }
    let target = kb_dir(folder, date);
    std::fs::create_dir_all(&target)?;
    for (name, _) in files {
        std::fs::rename(tmp.join(name), target.join(name))?;
    }
    std::fs::remove_dir_all(&tmp)
}

pub fn write_skipped(folder: &Path, date: NaiveDate, call: Call) -> std::io::Result<()> {
    let fm = Frontmatter {
        date,
        source: "none".into(),
        generated_by: "Ambient Context".into(),
        prompt_sha256: String::new(),
    };
    let files: Vec<(String, String)> = call
        .files()
        .iter()
        .map(|n| ((*n).to_string(), NOTHING.to_string()))
        .collect();
    write_call(folder, date, call, &files, &fm)?;
    record_call(
        folder,
        date,
        call,
        CallRecord {
            disposition: "skipped".into(),
            input_sha256: "none".into(),
            timeline_sha256: String::new(),
            prompt_sha256: String::new(),
            engine: String::new(),
            at: chrono::Local::now().to_rfc3339(),
        },
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CallRecord {
    pub disposition: String,
    pub input_sha256: String,
    pub timeline_sha256: String,
    pub prompt_sha256: String,
    pub engine: String,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Manifest {
    pub date: String,
    pub calls: BTreeMap<String, CallRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hashes {
    pub input: String,
    pub timeline: String,
    pub prompt: String,
}

fn manifest_path(folder: &Path, date: NaiveDate) -> PathBuf {
    kb_dir(folder, date).join("manifest.md")
}

pub fn read_manifest(folder: &Path, date: NaiveDate) -> Manifest {
    let mut manifest = Manifest::default();
    let Ok(text) = std::fs::read_to_string(manifest_path(folder, date)) else {
        return manifest;
    };
    for line in text.lines() {
        let Some((key, value)) = line.split_once(": ") else {
            continue;
        };
        if key == "date" {
            manifest.date = value.trim().to_string();
            continue;
        }
        let Some((call, field)) = key.split_once('.') else {
            continue;
        };
        let record = manifest.calls.entry(call.to_string()).or_default();
        let value = value.trim().to_string();
        match field {
            "disposition" => record.disposition = value,
            "input_sha256" => record.input_sha256 = value,
            "timeline_sha256" => record.timeline_sha256 = value,
            "prompt_sha256" => record.prompt_sha256 = value,
            "engine" => record.engine = value,
            "at" => record.at = value,
            _ => {}
        }
    }
    manifest
}

fn write_manifest(folder: &Path, date: NaiveDate, manifest: &Manifest) -> std::io::Result<()> {
    let mut out = format!("---\ndate: {}\n", date.format("%Y-%m-%d"));
    for (call, r) in &manifest.calls {
        out.push_str(&format!(
            "{call}.disposition: {}\n{call}.input_sha256: {}\n{call}.timeline_sha256: {}\n{call}.prompt_sha256: {}\n{call}.engine: {}\n{call}.at: {}\n",
            r.disposition, r.input_sha256, r.timeline_sha256, r.prompt_sha256, r.engine, r.at
        ));
    }
    out.push_str("---\n");
    std::fs::create_dir_all(kb_dir(folder, date))?;
    std::fs::write(manifest_path(folder, date), out)
}

pub fn record_call(
    folder: &Path,
    date: NaiveDate,
    call: Call,
    record: CallRecord,
) -> std::io::Result<()> {
    let mut manifest = read_manifest(folder, date);
    manifest.calls.insert(call.action().to_string(), record);
    write_manifest(folder, date, &manifest)
}

pub fn needs_ingest(folder: &Path, date: NaiveDate, call: Call, hashes: &Hashes) -> bool {
    let manifest = read_manifest(folder, date);
    let Some(record) = manifest.calls.get(call.action()) else {
        return true;
    };
    match record.disposition.as_str() {
        "accepted" => {
            record.input_sha256 != hashes.input
                || record.timeline_sha256 != hashes.timeline
                || record.prompt_sha256 != hashes.prompt
        }
        "skipped" => hashes.input != "none",
        _ => true,
    }
}

pub fn has_kb(folder: &Path, date: NaiveDate) -> bool {
    read_manifest(folder, date)
        .calls
        .values()
        .any(|r| r.disposition == "accepted")
}

fn strip_frontmatter(text: &str) -> &str {
    let t = text.trim_start();
    if !t.starts_with("---\n") {
        return text;
    }
    match t[4..].find("\n---\n") {
        Some(i) => t[4 + i + 5..].trim_start_matches('\n'),
        None => text,
    }
}

/// One KB file, the manifest, or the concatenated prompt view when `file`
/// is None. The Day view command lands in Task 11.
#[allow(dead_code)]
pub fn read_kb(folder: &Path, date: NaiveDate, file: Option<&str>) -> Option<String> {
    let dir = kb_dir(folder, date);
    match file {
        Some(name) if name == "manifest.md" || KB_FILES.contains(&name) => {
            std::fs::read_to_string(dir.join(name)).ok()
        }
        Some(_) => None,
        None => {
            if !dir.is_dir() {
                return None;
            }
            Some(kb_for_prompt(folder, date))
        }
    }
}

pub fn kb_for_prompt(folder: &Path, date: NaiveDate) -> String {
    let dir = kb_dir(folder, date);
    let mut out = String::new();
    for name in KB_FILES {
        out.push_str(&format!("# {name}\n\n"));
        match std::fs::read_to_string(dir.join(name)) {
            Ok(text) => out.push_str(strip_frontmatter(&text).trim()),
            Err(_) => out.push_str("(not ingested)"),
        }
        out.push_str("\n\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn date() -> chrono::NaiveDate {
        chrono::NaiveDate::from_ymd_opt(2026, 9, 2).unwrap()
    }

    const GOOD_MESSAGES: &str = "Some preamble the model added.\n<<<file: people.md>>>\n## Dan\nAsked for the notch state by Thursday in #empty-build 09:48-09:59 url: https://app.slack.com/x\n\n<<<file: commitments.md>>>\n## I agreed to\n- [ ] ship the notch state · with Dan · 09:48-09:59 · https://app.slack.com/x\n\n## Owed to me\nNothing evident.\n<<<reasoning>>>\nDan wrote directly; newsletters were skipped.\n";

    fn spans() -> Vec<(u32, u32)> {
        vec![(540, 570), (588, 599)]
    }

    #[test]
    fn split_finds_files_and_reasoning_and_ignores_preamble() {
        let split = split_output(GOOD_MESSAGES);
        assert_eq!(split.files.len(), 2);
        assert_eq!(split.files[0].0, "people.md");
        assert!(split.files[0].1.starts_with("## Dan"));
        assert_eq!(
            split.reasoning.as_deref(),
            Some("Dan wrote directly; newsletters were skipped.")
        );
    }

    #[test]
    fn split_unfences_a_fenced_reply() {
        let fenced = format!("```markdown\n{GOOD_MESSAGES}```");
        assert_eq!(split_output(&fenced).files.len(), 2);
    }

    #[test]
    fn a_good_split_validates() {
        validate(Call::Messages, &split_output(GOOD_MESSAGES), &spans()).unwrap();
    }

    #[test]
    fn a_missing_file_is_rejected() {
        let text = GOOD_MESSAGES.replace("<<<file: commitments.md>>>", "<<<file: promises.md>>>");
        assert_eq!(
            validate(Call::Messages, &split_output(&text), &spans()).unwrap_err(),
            Invalid::MissingFile("commitments.md".into())
        );
    }

    #[test]
    fn a_line_without_a_citation_is_rejected() {
        let text = GOOD_MESSAGES.replace(" 09:48-09:59 url: https://app.slack.com/x", "");
        assert!(matches!(
            validate(Call::Messages, &split_output(&text), &spans()).unwrap_err(),
            Invalid::NoCitation { file, .. } if file == "people.md"
        ));
    }

    #[test]
    fn a_citation_outside_the_timeline_is_rejected() {
        let text = GOOD_MESSAGES.replace("09:48-09:59", "14:00-14:30");
        assert!(matches!(
            validate(Call::Messages, &split_output(&text), &spans()).unwrap_err(),
            Invalid::CitationOutsideTimeline { .. }
        ));
    }

    #[test]
    fn nothing_evident_is_accepted_and_a_long_file_is_not() {
        let mut long = String::from("<<<file: reading.md>>>\n## Topic\n");
        for _ in 0..201 {
            long.push_str("- t · d · 3m · 09:00-09:30 · https://x\n");
        }
        assert!(matches!(
            validate(Call::Websites, &split_output(&long), &spans()).unwrap_err(),
            Invalid::TooLong { .. }
        ));
        assert!(validate(
            Call::Websites,
            &split_output("<<<file: reading.md>>>\nNothing evident.\n"),
            &spans()
        )
        .is_ok());
    }

    #[test]
    fn trim_drops_the_longest_bodies_first_and_keeps_headings() {
        let text = "## 09:00\u{2013}09:10 \u{00b7} A\n\nshort\n\n## 09:10\u{2013}09:20 \u{00b7} B\n\nfile: /x\n\nthis body is much longer than the other one by a wide margin\nsecond line\n";
        let (out, trimmed) = trim_input(text, 90);
        assert_eq!(trimmed, 1);
        assert!(out.contains("## 09:10\u{2013}09:20 \u{00b7} B\n\nfile: /x\n\n[trimmed 2 lines]\n"));
        assert!(out.contains("short"));
        let (same, none) = trim_input(text, 10_000);
        assert_eq!((same.as_str(), none), (text, 0));
    }

    #[test]
    fn write_call_adds_frontmatter_and_leaves_no_tmp_folder() {
        let dir = tempdir().unwrap();
        let fm = Frontmatter {
            date: date(),
            source: "messages.md".into(),
            generated_by: "stub".into(),
            prompt_sha256: "abc".into(),
        };
        let files = split_output(GOOD_MESSAGES).files;
        write_call(dir.path(), date(), Call::Messages, &files, &fm).unwrap();
        let people = std::fs::read_to_string(kb_dir(dir.path(), date()).join("people.md")).unwrap();
        assert!(people.starts_with(
            "---\ndate: 2026-09-02\nkind: kb\nsource: messages.md\ngenerated_by: stub\nprompt_sha256: abc\n---\n\n## Dan"
        ));
        assert!(!dir
            .path()
            .join("KB")
            .join(".tmp-2026-09-02-messages")
            .exists());
        assert!(!has_kb(dir.path(), date()), "no accepted call recorded yet");
    }

    #[test]
    fn manifest_round_trips_and_drives_needs_ingest() {
        let dir = tempdir().unwrap();
        let hashes = Hashes {
            input: "i1".into(),
            timeline: "t1".into(),
            prompt: "p1".into(),
        };
        assert!(
            needs_ingest(dir.path(), date(), Call::Apps, &hashes),
            "no manifest"
        );
        record_call(
            dir.path(),
            date(),
            Call::Apps,
            CallRecord {
                disposition: "accepted".into(),
                input_sha256: "i1".into(),
                timeline_sha256: "t1".into(),
                prompt_sha256: "p1".into(),
                engine: "stub".into(),
                at: "2026-09-03T06:00:00+10:00".into(),
            },
        )
        .unwrap();
        assert!(!needs_ingest(dir.path(), date(), Call::Apps, &hashes));
        assert!(
            needs_ingest(dir.path(), date(), Call::Messages, &hashes),
            "other call absent"
        );
        assert!(
            needs_ingest(
                dir.path(),
                date(),
                Call::Apps,
                &Hashes {
                    input: "i2".into(),
                    ..hashes.clone()
                }
            ),
            "input changed"
        );
        assert!(has_kb(dir.path(), date()));
        let manifest = read_manifest(dir.path(), date());
        assert_eq!(manifest.calls["ingest_apps"].engine, "stub");
        let text = std::fs::read_to_string(kb_dir(dir.path(), date()).join("manifest.md")).unwrap();
        assert!(text.contains("ingest_apps.disposition: accepted\n"));
    }

    #[test]
    fn write_skipped_writes_nothing_evident_with_source_none() {
        let dir = tempdir().unwrap();
        write_skipped(dir.path(), date(), Call::Messages).unwrap();
        let text =
            std::fs::read_to_string(kb_dir(dir.path(), date()).join("commitments.md")).unwrap();
        assert!(text.contains("source: none\n"));
        assert!(text.trim_end().ends_with("Nothing evident."));
        assert_eq!(
            read_manifest(dir.path(), date()).calls["ingest_messages"].disposition,
            "skipped"
        );
    }

    #[test]
    fn kb_for_prompt_concatenates_without_frontmatter() {
        let dir = tempdir().unwrap();
        let fm = Frontmatter {
            date: date(),
            source: "messages.md".into(),
            generated_by: "stub".into(),
            prompt_sha256: "abc".into(),
        };
        write_call(
            dir.path(),
            date(),
            Call::Messages,
            &split_output(GOOD_MESSAGES).files,
            &fm,
        )
        .unwrap();
        let out = kb_for_prompt(dir.path(), date());
        assert!(out.starts_with("# people.md\n\n## Dan"));
        assert!(out.contains("# commitments.md\n\n## I agreed to"));
        assert!(out.contains("# threads.md\n\n(not ingested)\n"));
        assert!(!out.contains("prompt_sha256"));
    }
}
