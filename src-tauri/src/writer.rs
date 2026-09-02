use crate::route::{self, Kind};
use crate::rules::Rules;
use crate::segment::Block;
use chrono::{Datelike, NaiveDate};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Remembers every line already written today, so a line costs tokens once
/// per day no matter how many blocks it appears in. Cross-block repetition
/// was 73% of a measured real day.
pub struct DayDedup {
    date: Option<NaiveDate>,
    seen: HashSet<u64>,
    // Skeletons (digits normalised) of lines where a repeat with different
    // numbers is a re-capture: the same tweet with its "ago" counter ticked,
    // the same story row with a new vote count.
    skeletons: HashSet<u64>,
}

impl DayDedup {
    pub fn new() -> Self {
        DayDedup {
            date: None,
            seen: HashSet::new(),
            skeletons: HashSet::new(),
        }
    }

    fn hash(line: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        line.hash(&mut hasher);
        hasher.finish()
    }

    /// Moves to `date` if it is not already there, seeding the seen-set from
    /// any existing file for that day so a restart does not re-admit
    /// everything written before it.
    fn roll_to(&mut self, folder: &Path, date: NaiveDate) {
        if self.date == Some(date) {
            return;
        }
        self.date = Some(date);
        self.seen.clear();
        self.skeletons.clear();
        for file in [DayFile::Apps, DayFile::Messages] {
            let Ok(existing) = fs::read_to_string(file.path(folder, date)) else {
                continue;
            };
            for line in existing.lines() {
                if line.is_empty()
                    || line == "---"
                    || line.starts_with("## ")
                    || line.starts_with("file: ")
                    || line.starts_with("url: ")
                    || line.starts_with("routed: ")
                    || line.starts_with("date: ")
                    || line.starts_with("kind: ")
                    || line.starts_with("captured_by: ")
                {
                    continue;
                }
                self.seen.insert(Self::hash(line));
                if crate::prune::is_skeleton_dedupable(line) {
                    self.skeletons
                        .insert(Self::hash(&crate::prune::skeleton(line)));
                }
            }
        }
    }

    /// The block's lines that today has not recorded yet, marking them seen.
    fn novel_lines(&mut self, folder: &Path, block: &Block) -> Vec<String> {
        let date = block.start.date_naive();
        self.roll_to(folder, date);
        // A deleted day folder means the user wants a fresh start; remembering
        // its lines would recreate it as bare headings.
        if !self.seen.is_empty() && !day_dir(folder, date).exists() {
            self.seen.clear();
            self.skeletons.clear();
        }
        block
            .lines
            .iter()
            .filter(|line| {
                if !self.seen.insert(Self::hash(line)) {
                    return false;
                }
                if crate::prune::is_skeleton_dedupable(line) {
                    return self
                        .skeletons
                        .insert(Self::hash(&crate::prune::skeleton(line)));
                }
                true
            })
            .cloned()
            .collect()
    }

    /// Forgets everything, for a folder change: what the old folder had
    /// recorded says nothing about the new one.
    pub fn reset(&mut self) {
        self.date = None;
        self.seen.clear();
        self.skeletons.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DayFile {
    Apps,
    Websites,
    Messages,
}

impl DayFile {
    pub fn all() -> [DayFile; 3] {
        [DayFile::Apps, DayFile::Websites, DayFile::Messages]
    }
    pub fn file_name(self) -> &'static str {
        match self {
            DayFile::Apps => "apps.md",
            DayFile::Websites => "websites.md",
            DayFile::Messages => "messages.md",
        }
    }
    pub fn kind_name(self) -> &'static str {
        match self {
            DayFile::Apps => "apps",
            DayFile::Websites => "websites",
            DayFile::Messages => "messages",
        }
    }
    pub fn from_name(name: &str) -> Option<DayFile> {
        match name {
            "apps" | "apps.md" => Some(DayFile::Apps),
            "websites" | "websites.md" => Some(DayFile::Websites),
            "messages" | "messages.md" => Some(DayFile::Messages),
            _ => None,
        }
    }
    pub fn path(self, folder: &Path, date: NaiveDate) -> PathBuf {
        day_dir(folder, date).join(self.file_name())
    }
}

pub fn days_dir(folder: &Path) -> PathBuf {
    folder.join("Days")
}

pub fn day_dir(folder: &Path, date: NaiveDate) -> PathBuf {
    days_dir(folder).join(format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        date.month(),
        date.day()
    ))
}

fn frontmatter(date: NaiveDate, file: DayFile) -> String {
    let mut out = format!(
        "---\ndate: {:04}-{:02}-{:02}\nkind: {}\ncaptured_by: Ambient Context {}\n---\n",
        date.year(),
        date.month(),
        date.day(),
        file.kind_name(),
        env!("CARGO_PKG_VERSION")
    );
    if file == DayFile::Websites {
        out.push_str("\n| start | end | app | domain | title | url |\n| --- | --- | --- | --- | --- | --- |\n");
    }
    out
}

/// The two output knobs the settings page exposes. `max_block_chars` of 0
/// is unlimited, which is what the default must be for this release to
/// change nothing until it is touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shape {
    pub max_block_chars: usize,
    pub write_references: bool,
}

impl Default for Shape {
    fn default() -> Self {
        Shape {
            max_block_chars: 0,
            write_references: true,
        }
    }
}

/// A pipe inside a cell would split the row.
pub fn escape_cell(text: &str) -> String {
    text.replace('|', "\\|")
}

pub fn render_website_row(block: &Block) -> String {
    let url = block.url.clone().unwrap_or_default();
    let domain = crate::rules::domain_of(&url).unwrap_or_default();
    format!(
        "| {} | {} | {} | {} | {} | {} |\n",
        block.start.format("%H:%M"),
        block.end.format("%H:%M"),
        escape_cell(&block.app),
        escape_cell(&domain),
        escape_cell(block.title.as_deref().unwrap_or("")),
        escape_cell(&url),
    )
}

/// Renders a block's heading and references, with `lines` as the body:
/// the caller decides which lines are worth writing (usually the novel
/// ones). A block with no novel lines still renders, because the heading
/// is the day's timeline even when the content was all seen before.
pub fn render_block(block: &Block, lines: &[String], shape: Shape) -> String {
    let mut out = String::new();
    out.push_str("\n## ");
    out.push_str(&block.start.format("%H:%M").to_string());
    out.push('\u{2013}');
    out.push_str(&block.end.format("%H:%M").to_string());
    out.push_str(" \u{00b7} ");
    out.push_str(&block.app);
    if let Some(title) = &block.title {
        if !title.is_empty() {
            out.push_str(" \u{00b7} ");
            out.push_str(title);
        }
    }
    out.push_str("\n\n");
    // The reference outranks the scraped text: it points at the real
    // document, which the consuming LLM can open in full.
    if shape.write_references {
        if let Some(document) = &block.document {
            out.push_str("file: ");
            out.push_str(document);
            out.push('\n');
        }
        if let Some(url) = &block.url {
            out.push_str("url: ");
            out.push_str(url);
            out.push('\n');
        }
        if block.document.is_some() || block.url.is_some() {
            out.push('\n');
        }
    }
    // A headings-only block keeps its place in the timeline and its
    // reference, and gives up its text.
    if block.headings_only {
        return out;
    }
    let mut written = 0usize;
    for line in lines {
        if shape.max_block_chars > 0 && written >= shape.max_block_chars {
            out.push_str("[truncated]\n");
            break;
        }
        out.push_str(line);
        out.push('\n');
        written += line.chars().count() + 1;
    }
    out
}

/// The heading alone, then where the body went. References are left off:
/// the website row or the messages block carries them.
fn render_routed(block: &Block, kind: Kind) -> String {
    let mut out = render_block(
        block,
        &[],
        Shape {
            max_block_chars: 0,
            write_references: false,
        },
    );
    if let Some(name) = kind.routed_name() {
        out.push_str("routed: ");
        out.push_str(name);
        out.push('\n');
    }
    out
}

fn append_to(path: &Path, date: NaiveDate, file: DayFile, text: &str) -> std::io::Result<()> {
    let is_new = !path.exists();
    let mut handle = OpenOptions::new().create(true).append(true).open(path)?;
    if is_new {
        handle.write_all(frontmatter(date, file).as_bytes())?;
    }
    handle.write_all(text.as_bytes())
}

/// Appends one block to the files for the block's own start date. The
/// heading always goes to apps.md; the body goes where the kind says.
pub fn append_block(
    folder: &Path,
    block: &Block,
    dedup: &mut DayDedup,
    shape: Shape,
    rules: &Rules,
) -> std::io::Result<()> {
    let date = block.start.date_naive();
    let kind = route::kind(
        rules,
        &block.app,
        block.title.as_deref(),
        block.url.as_deref(),
    );
    let apps = DayFile::Apps.path(folder, date);

    match kind {
        Kind::App => {
            let novel = if block.headings_only {
                Vec::new()
            } else {
                dedup.novel_lines(folder, block)
            };
            fs::create_dir_all(day_dir(folder, date))?;
            append_to(
                &apps,
                date,
                DayFile::Apps,
                &render_block(block, &novel, shape),
            )?;
        }
        Kind::Website => {
            fs::create_dir_all(day_dir(folder, date))?;
            append_to(&apps, date, DayFile::Apps, &render_routed(block, kind))?;
            append_to(
                &DayFile::Websites.path(folder, date),
                date,
                DayFile::Websites,
                &render_website_row(block),
            )?;
        }
        Kind::Message => {
            let cleaned = Block {
                lines: crate::prune::for_kind(kind, block.lines.clone()),
                ..block.clone()
            };
            let novel = if cleaned.headings_only {
                Vec::new()
            } else {
                dedup.novel_lines(folder, &cleaned)
            };
            fs::create_dir_all(day_dir(folder, date))?;
            append_to(&apps, date, DayFile::Apps, &render_routed(block, kind))?;
            append_to(
                &DayFile::Messages.path(folder, date),
                date,
                DayFile::Messages,
                &render_block(&cleaned, &novel, shape),
            )?;
        }
    }
    ensure_agents_file(folder)
}

/// Content hashes of `AGENTS.md` as earlier versions of the app shipped
/// it. A file matching one of these is the app's own copy, untouched, and
/// can be replaced with the current version. Anything else belongs to the
/// user.
const PREVIOUS_BUNDLED_AGENTS: &[&str] = &[
    "3102846b796ab65642806000543c289e49131d53001b53cada23b0f88467f0af",
    "5302950a122a3433f6a4dbbb0a285f83397d516bf9b7a1ba17619384637dd2ae",
];

fn is_bundled_agents_file(text: &str, current: &str) -> bool {
    text == current
        || PREVIOUS_BUNDLED_AGENTS.contains(&crate::ledger::sha256_of(text.as_bytes()).as_str())
}

/// Writes AGENTS.md into the capture folder so the folder explains itself
/// to whatever LLM reads it. A file the user has edited is never
/// overwritten: only a copy this app wrote, byte for byte, is replaced.
/// Everything else gets the new version beside it as `AGENTS.md.new`, once,
/// with a line on stderr saying so.
pub fn ensure_agents_file(folder: &Path) -> std::io::Result<()> {
    let path = folder.join("AGENTS.md");
    let current = include_str!("../assets/AGENTS.md");
    let Ok(existing) = fs::read_to_string(&path) else {
        return fs::write(&path, current);
    };
    if existing == current {
        return Ok(());
    }
    if is_bundled_agents_file(&existing, current) {
        return fs::write(&path, current);
    }
    let beside = folder.join("AGENTS.md.new");
    if fs::read_to_string(&beside).ok().as_deref() != Some(current) {
        fs::write(&beside, current)?;
        eprintln!(
            "[writer] AGENTS.md has been edited, so this version was written to {} instead.",
            beside.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Rules;
    use chrono::{Local, TimeZone};
    use tempfile::tempdir;

    fn date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 8, 25).unwrap()
    }

    fn block(
        app: &str,
        title: &str,
        url: Option<&str>,
        document: Option<&str>,
        minute: u32,
        end_minute: u32,
        lines: &[&str],
    ) -> Block {
        Block {
            app: app.to_string(),
            title: Some(title.to_string()),
            document: document.map(str::to_string),
            url: url.map(str::to_string),
            start: Local.with_ymd_and_hms(2026, 8, 25, 9, minute, 0).unwrap(),
            end: Local
                .with_ymd_and_hms(2026, 8, 25, 9, end_minute, 0)
                .unwrap(),
            lines: lines.iter().map(|s| s.to_string()).collect(),
            headings_only: false,
        }
    }

    fn zed() -> Block {
        block(
            "Zed",
            "writer.rs",
            None,
            Some("/Users/x/writer.rs"),
            14,
            41,
            &["fn append_block"],
        )
    }
    fn arc() -> Block {
        block(
            "Arc",
            "Tauri | system tray",
            Some("https://v2.tauri.app/learn/system-tray/"),
            None,
            41,
            48,
            &["Tray icons on macOS"],
        )
    }
    fn slack() -> Block {
        block(
            "Slack",
            "#empty-build",
            None,
            None,
            48,
            59,
            &["dan: shipping the notch state thursday"],
        )
    }

    fn write_all(dir: &Path) {
        let mut dedup = DayDedup::new();
        for b in [zed(), arc(), slack()] {
            append_block(dir, &b, &mut dedup, Shape::default(), &Rules::default()).unwrap();
        }
    }

    fn read(dir: &Path, file: DayFile) -> String {
        fs::read_to_string(file.path(dir, date())).unwrap()
    }

    fn block_at(hour: u32, minute: u32, end_minute: u32) -> Block {
        Block {
            app: "Zed".to_string(),
            title: Some("YN-102".to_string()),
            document: None,
            url: None,
            start: Local
                .with_ymd_and_hms(2026, 8, 25, hour, minute, 0)
                .unwrap(),
            end: Local
                .with_ymd_and_hms(2026, 8, 25, hour, end_minute, 0)
                .unwrap(),
            lines: vec!["read the issue".to_string()],
            headings_only: false,
        }
    }

    #[test]
    fn message_bodies_are_pruned_of_mail_chrome_before_writing() {
        let dir = tempdir().unwrap();
        let mail = block(
            "Mail",
            "All Inboxes",
            None,
            None,
            10,
            12,
            &[
                "7:09 am",
                "Reply-To: \u{fffc}",
                "Patient letter regarding Mr Smith",
            ],
        );
        append_block(
            dir.path(),
            &mail,
            &mut DayDedup::new(),
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();
        let messages = read(dir.path(), DayFile::Messages);
        assert!(messages.contains("Patient letter regarding Mr Smith"));
        assert!(!messages.contains("7:09 am"));
        assert!(!messages.contains("Reply-To"));
    }

    #[test]
    fn day_dir_is_days_slash_date() {
        assert_eq!(
            day_dir(Path::new("/tmp/x"), date()),
            PathBuf::from("/tmp/x/Days/2026-08-25")
        );
        assert_eq!(
            DayFile::Apps.path(Path::new("/tmp/x"), date()),
            PathBuf::from("/tmp/x/Days/2026-08-25/apps.md")
        );
    }

    #[test]
    fn three_blocks_land_in_three_files() {
        let dir = tempdir().unwrap();
        write_all(dir.path());

        let apps = read(dir.path(), DayFile::Apps);
        assert!(
            apps.starts_with("---\ndate: 2026-08-25\nkind: apps\ncaptured_by: Ambient Context ")
        );
        assert!(apps.contains("## 09:14\u{2013}09:41 \u{00b7} Zed \u{00b7} writer.rs\n\nfile: /Users/x/writer.rs\n\nfn append_block\n"));
        assert!(apps.contains(
            "## 09:41\u{2013}09:48 \u{00b7} Arc \u{00b7} Tauri | system tray\n\nrouted: websites\n"
        ));
        assert!(apps.contains(
            "## 09:48\u{2013}09:59 \u{00b7} Slack \u{00b7} #empty-build\n\nrouted: messages\n"
        ));
        assert!(!apps.contains("Tray icons on macOS"));
        assert!(!apps.contains("dan: shipping"));

        let websites = read(dir.path(), DayFile::Websites);
        assert!(websites.contains("kind: websites\n"));
        assert!(websites.contains(
            "| start | end | app | domain | title | url |\n| --- | --- | --- | --- | --- | --- |\n"
        ));
        assert!(websites.contains("| 09:41 | 09:48 | Arc | v2.tauri.app | Tauri \\| system tray | https://v2.tauri.app/learn/system-tray/ |\n"));

        let messages = read(dir.path(), DayFile::Messages);
        assert!(messages.contains("kind: messages\n"));
        assert!(messages.contains("## 09:48\u{2013}09:59 \u{00b7} Slack \u{00b7} #empty-build\n\ndan: shipping the notch state thursday\n"));
    }

    #[test]
    fn a_website_block_does_not_enter_the_dedup_set() {
        let dir = tempdir().unwrap();
        let mut dedup = DayDedup::new();
        let mut page = arc();
        page.lines = vec!["shared sentence here".to_string()];
        append_block(
            dir.path(),
            &page,
            &mut dedup,
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();
        let mut editor = zed();
        editor.lines = vec!["shared sentence here".to_string()];
        append_block(
            dir.path(),
            &editor,
            &mut dedup,
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();
        assert!(read(dir.path(), DayFile::Apps).contains("shared sentence here"));
    }

    #[test]
    fn a_restart_reseeds_from_apps_and_messages_together() {
        let dir = tempdir().unwrap();
        write_all(dir.path());
        let mut fresh = DayDedup::new();
        let mut again = zed();
        again.lines = vec![
            "fn append_block".into(),
            "dan: shipping the notch state thursday".into(),
            "new line".into(),
        ];
        append_block(
            dir.path(),
            &again,
            &mut fresh,
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();
        let apps = read(dir.path(), DayFile::Apps);
        assert_eq!(apps.matches("fn append_block").count(), 1);
        assert!(
            !apps.contains("dan: shipping"),
            "seen in messages.md already"
        );
        assert!(apps.contains("new line"));
    }

    #[test]
    fn a_headings_only_message_block_writes_headings_to_both_files() {
        let dir = tempdir().unwrap();
        let mut quiet = slack();
        quiet.headings_only = true;
        append_block(
            dir.path(),
            &quiet,
            &mut DayDedup::new(),
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();
        assert!(read(dir.path(), DayFile::Apps).contains("routed: messages"));
        let messages = read(dir.path(), DayFile::Messages);
        assert!(messages.contains("## 09:48"));
        assert!(!messages.contains("dan: shipping"));
    }

    #[test]
    fn a_website_block_with_no_url_has_empty_cells() {
        let dir = tempdir().unwrap();
        let mut page = arc();
        page.url = None;
        append_block(
            dir.path(),
            &page,
            &mut DayDedup::new(),
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();
        assert!(read(dir.path(), DayFile::Websites)
            .contains("| 09:41 | 09:48 | Arc |  | Tauri \\| system tray |  |\n"));
    }

    #[test]
    fn a_deleted_day_folder_means_a_fresh_start() {
        let dir = tempdir().unwrap();
        let mut dedup = DayDedup::new();
        append_block(
            dir.path(),
            &zed(),
            &mut dedup,
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();
        fs::remove_dir_all(day_dir(dir.path(), date())).unwrap();
        append_block(
            dir.path(),
            &zed(),
            &mut dedup,
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();
        assert!(read(dir.path(), DayFile::Apps).contains("fn append_block"));
    }

    #[test]
    fn renders_a_heading_with_time_range_app_and_title() {
        let out = render_block(&zed(), &zed().lines, Shape::default());
        assert!(out.contains("## 09:14\u{2013}09:41 \u{00b7} Zed \u{00b7} writer.rs"));
    }

    #[test]
    fn renders_document_and_url_references_before_the_text() {
        let mut block = block_at(9, 14, 41);
        block.document = Some("/Users/x/report.pdf".to_string());
        block.url = Some("https://v2.tauri.app/".to_string());
        let out = render_block(&block, &block.lines.clone(), Shape::default());
        assert!(out.contains("file: /Users/x/report.pdf\n"));
        assert!(out.contains("url: https://v2.tauri.app/\n"));
        assert!(
            out.find("file:").unwrap() < out.find("read the issue").unwrap(),
            "references come before the scraped text"
        );
    }

    #[test]
    fn renders_without_a_title_when_there_is_none() {
        let mut block = block_at(9, 14, 41);
        block.title = None;
        let out = render_block(&block, &block.lines.clone(), Shape::default());
        assert!(out.contains("\u{00b7} Zed\n"));
    }

    #[test]
    fn creates_the_folder_if_it_is_missing() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("a").join("b");
        append_block(
            &nested,
            &block_at(9, 14, 41),
            &mut DayDedup::new(),
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();
        assert!(day_dir(&nested, date()).exists());
    }

    #[test]
    fn a_line_is_written_once_per_day_but_headings_always_appear() {
        let dir = tempdir().unwrap();
        let mut dedup = DayDedup::new();
        append_block(
            dir.path(),
            &block_at(9, 14, 41),
            &mut dedup,
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();
        append_block(
            dir.path(),
            &block_at(10, 0, 20),
            &mut dedup,
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();

        let contents = read(dir.path(), DayFile::Apps);

        assert_eq!(contents.matches("read the issue").count(), 1);
        assert_eq!(contents.matches("## ").count(), 2, "both headings kept");
    }

    #[test]
    fn a_fresh_dedup_is_seeded_from_the_existing_day_file() {
        let dir = tempdir().unwrap();
        append_block(
            dir.path(),
            &block_at(9, 14, 41),
            &mut DayDedup::new(),
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();
        append_block(
            dir.path(),
            &block_at(10, 0, 20),
            &mut DayDedup::new(),
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();

        let contents = read(dir.path(), DayFile::Apps);
        assert_eq!(contents.matches("read the issue").count(), 1);
    }

    #[test]
    fn novel_lines_still_write_alongside_repeated_ones() {
        let dir = tempdir().unwrap();
        let mut dedup = DayDedup::new();
        append_block(
            dir.path(),
            &block_at(9, 14, 41),
            &mut dedup,
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();
        let mut second = block_at(10, 0, 20);
        second.lines = vec![
            "read the issue".to_string(),
            "drafted the reply".to_string(),
        ];
        append_block(
            dir.path(),
            &second,
            &mut dedup,
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();

        let contents = read(dir.path(), DayFile::Apps);
        assert_eq!(contents.matches("read the issue").count(), 1);
        assert_eq!(contents.matches("drafted the reply").count(), 1);
    }

    #[test]
    fn reset_forgets_seen_lines() {
        let dir_a = tempdir().unwrap();
        let dir_b = tempdir().unwrap();
        let mut dedup = DayDedup::new();
        append_block(
            dir_a.path(),
            &block_at(9, 14, 41),
            &mut dedup,
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();
        dedup.reset();
        append_block(
            dir_b.path(),
            &block_at(10, 0, 20),
            &mut dedup,
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();

        let contents = read(dir_b.path(), DayFile::Apps);
        assert!(contents.contains("read the issue"));
    }

    #[test]
    fn digit_varying_recaptures_write_once_per_day() {
        let dir = tempdir().unwrap();
        let mut dedup = DayDedup::new();
        let tweet_v1 = "Dan Verified account @dan 5 hours ago so 3 things broke today".to_string();
        let tweet_v2 = "Dan Verified account @dan 6 hours ago so 3 things broke today".to_string();

        let mut first = block_at(9, 14, 41);
        first.lines = vec![tweet_v1.clone()];
        append_block(
            dir.path(),
            &first,
            &mut dedup,
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();

        let mut second = block_at(10, 0, 20);
        second.lines = vec![tweet_v2];
        append_block(
            dir.path(),
            &second,
            &mut dedup,
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();

        let contents = read(dir.path(), DayFile::Apps);
        assert!(contents.contains(&tweet_v1));
        assert!(
            !contents.contains("6 hours ago"),
            "the re-capture is dropped"
        );
    }

    #[test]
    fn writes_agents_md_into_the_folder() {
        let dir = tempdir().unwrap();
        append_block(
            dir.path(),
            &block_at(9, 14, 41),
            &mut DayDedup::new(),
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();
        let text = fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
        assert!(text.contains("## Summaries"));
        assert!(text.contains("## Ledger"));
    }

    #[test]
    fn an_edited_agents_file_is_kept_and_the_new_version_lands_beside_it() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("AGENTS.md");
        let theirs = "# Reading this folder\n\nMy own notes about this record.\n";
        fs::write(&path, theirs).unwrap();

        ensure_agents_file(dir.path()).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            theirs,
            "their file stands"
        );
        let beside = fs::read_to_string(dir.path().join("AGENTS.md.new")).unwrap();
        assert!(beside.contains("## Ledger"));

        ensure_agents_file(dir.path()).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), theirs);
        assert_eq!(
            fs::read_to_string(dir.path().join("AGENTS.md.new")).unwrap(),
            beside
        );
    }

    #[test]
    fn only_a_copy_this_app_wrote_counts_as_its_own() {
        let current = include_str!("../assets/AGENTS.md");
        assert!(is_bundled_agents_file(current, current));
        assert!(!is_bundled_agents_file("# Reading this folder\n", current));
    }

    #[test]
    fn a_current_agents_file_keeps_the_users_edits() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("AGENTS.md");
        ensure_agents_file(dir.path()).unwrap();
        let edited = format!("{}\n\nMy own note.\n", fs::read_to_string(&path).unwrap());
        fs::write(&path, &edited).unwrap();
        ensure_agents_file(dir.path()).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), edited);
    }

    #[test]
    fn an_untouched_agents_file_is_left_exactly_as_it_is() {
        let dir = tempdir().unwrap();
        ensure_agents_file(dir.path()).unwrap();
        ensure_agents_file(dir.path()).unwrap();
        assert!(
            !dir.path().join("AGENTS.md.new").exists(),
            "nothing to write beside it"
        );
    }

    #[test]
    fn a_headings_only_block_writes_its_heading_and_references_and_no_body() {
        let mut block = block_at(9, 14, 41);
        block.headings_only = true;
        block.url = Some("https://news.ycombinator.com/".to_string());
        let out = render_block(&block, &block.lines.clone(), Shape::default());
        assert!(out.contains("09:14"));
        assert!(out.contains("url: https://news.ycombinator.com/"));
        assert!(!out.contains("read the issue"));
    }

    #[test]
    fn a_headings_only_block_does_not_consume_its_lines_from_the_day_dedup() {
        let dir = tempdir().unwrap();
        let mut dedup = DayDedup::new();
        let mut quiet = block_at(9, 0, 30);
        quiet.headings_only = true;
        append_block(
            dir.path(),
            &quiet,
            &mut dedup,
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();
        let loud = block_at(10, 0, 30);
        append_block(
            dir.path(),
            &loud,
            &mut dedup,
            Shape::default(),
            &Rules::default(),
        )
        .unwrap();
        let written = read(dir.path(), DayFile::Apps);
        assert_eq!(written.matches("read the issue").count(), 1);
    }

    #[test]
    fn a_body_over_the_limit_is_truncated() {
        let mut block = block_at(9, 14, 41);
        block.lines = (0..10)
            .map(|i| format!("line {i} of a fairly long body"))
            .collect();
        let shape = Shape {
            max_block_chars: 20,
            write_references: true,
        };
        let out = render_block(&block, &block.lines.clone(), shape);
        assert!(out.ends_with("[truncated]\n"));
        assert!(out.contains("line 0"));
        assert!(!out.contains("line 9"));
    }

    #[test]
    fn write_references_false_drops_the_reference_lines() {
        let mut block = block_at(9, 14, 41);
        block.document = Some("/Users/x/report.pdf".to_string());
        block.url = Some("https://v2.tauri.app/".to_string());
        let shape = Shape {
            max_block_chars: 0,
            write_references: false,
        };
        let out = render_block(&block, &block.lines.clone(), shape);
        assert!(!out.contains("file:"));
        assert!(!out.contains("url:"));
        assert!(out.contains("read the issue"));
    }
}
