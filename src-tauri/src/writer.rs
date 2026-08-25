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
        if let Ok(existing) = fs::read_to_string(file_path(folder, date)) {
            for line in existing.lines() {
                if line.is_empty()
                    || line == "---"
                    || line.starts_with("## ")
                    || line.starts_with("file: ")
                    || line.starts_with("url: ")
                    || line.starts_with("date: ")
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
        // A deleted day file means the user wants a fresh start; remembering
        // its lines would recreate it as bare headings.
        if !self.seen.is_empty() && !file_path(folder, date).exists() {
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

pub fn file_path(folder: &Path, date: NaiveDate) -> PathBuf {
    folder.join(format!(
        "{:04}-{:02}-{:02}.md",
        date.year(),
        date.month(),
        date.day()
    ))
}

fn frontmatter(date: NaiveDate) -> String {
    format!(
        "---\ndate: {:04}-{:02}-{:02}\ncaptured_by: Ambient Context {}\n---\n",
        date.year(),
        date.month(),
        date.day(),
        env!("CARGO_PKG_VERSION")
    )
}

/// Renders a block's heading and references, with `lines` as the body:
/// the caller decides which lines are worth writing (usually the novel
/// ones). A block with no novel lines still renders, because the heading
/// is the day's timeline even when the content was all seen before.
pub fn render_block(block: &Block, lines: &[String]) -> String {
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
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Appends one block to the file for the block's own start date, creating the
/// folder and the file with frontmatter if they do not exist yet. Only lines
/// the day has not already recorded are written.
pub fn append_block(
    folder: &Path,
    block: &Block,
    dedup: &mut DayDedup,
) -> std::io::Result<()> {
    fs::create_dir_all(folder)?;
    let date = block.start.date_naive();
    let path = file_path(folder, date);
    let is_new = !path.exists();
    let novel = dedup.novel_lines(folder, block);

    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    if is_new {
        file.write_all(frontmatter(date).as_bytes())?;
    }
    file.write_all(render_block(block, &novel).as_bytes())?;
    ensure_agents_file(folder)?;
    Ok(())
}

/// Writes AGENTS.md into the capture folder if it is not already there, so
/// the folder explains itself to whatever LLM reads it. Never overwrites:
/// the user may have edited it.
pub fn ensure_agents_file(folder: &Path) -> std::io::Result<()> {
    let path = folder.join("AGENTS.md");
    if path.exists() {
        return Ok(());
    }
    fs::write(path, include_str!("../assets/AGENTS.md"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, TimeZone};
    use tempfile::tempdir;

    fn block_at(hour: u32, minute: u32, end_minute: u32) -> Block {
        Block {
            app: "Linear".to_string(),
            title: Some("YN-102".to_string()),
            document: None,
            url: None,
            start: Local.with_ymd_and_hms(2026, 8, 25, hour, minute, 0).unwrap(),
            end: Local
                .with_ymd_and_hms(2026, 8, 25, hour, end_minute, 0)
                .unwrap(),
            lines: vec!["read the issue".to_string()],
        }
    }

    #[test]
    fn renders_document_and_url_references_before_the_text() {
        let mut block = block_at(9, 14, 41);
        block.document = Some("/Users/x/report.pdf".to_string());
        block.url = Some("https://v2.tauri.app/".to_string());
        let out = render_block(&block, &block.lines.clone());
        assert!(out.contains("file: /Users/x/report.pdf\n"));
        assert!(out.contains("url: https://v2.tauri.app/\n"));
        assert!(
            out.find("file:").unwrap() < out.find("read the issue").unwrap(),
            "references come before the scraped text"
        );
    }

    #[test]
    fn file_path_is_the_iso_date() {
        let path = file_path(
            Path::new("/tmp/x"),
            NaiveDate::from_ymd_opt(2026, 8, 5).unwrap(),
        );
        assert_eq!(path, PathBuf::from("/tmp/x/2026-08-05.md"));
    }

    #[test]
    fn renders_a_heading_with_time_range_app_and_title() {
        let block = block_at(9, 14, 41);
        let out = render_block(&block, &block.lines.clone());
        assert!(out.contains("## 09:14\u{2013}09:41 \u{00b7} Linear \u{00b7} YN-102"));
        assert!(out.contains("read the issue"));
    }

    #[test]
    fn renders_without_a_title_when_there_is_none() {
        let mut block = block_at(9, 14, 41);
        block.title = None;
        let out = render_block(&block, &block.lines.clone());
        assert!(out.contains("\u{00b7} Linear\n"));
    }

    #[test]
    fn creates_the_file_with_frontmatter_once() {
        let dir = tempdir().unwrap();
        let mut dedup = DayDedup::new();
        append_block(dir.path(), &block_at(9, 14, 41), &mut dedup).unwrap();
        append_block(dir.path(), &block_at(10, 0, 20), &mut dedup).unwrap();

        let path = file_path(dir.path(), NaiveDate::from_ymd_opt(2026, 8, 25).unwrap());
        let contents = fs::read_to_string(path).unwrap();

        assert_eq!(contents.matches("captured_by:").count(), 1);
        assert!(contents.starts_with("---\ndate: 2026-08-25\n"));
        assert_eq!(contents.matches("## ").count(), 2);
    }

    #[test]
    fn creates_the_folder_if_it_is_missing() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("a").join("b");
        append_block(&nested, &block_at(9, 14, 41), &mut DayDedup::new()).unwrap();
        assert!(nested.exists());
    }

    #[test]
    fn a_line_is_written_once_per_day_but_headings_always_appear() {
        let dir = tempdir().unwrap();
        let mut dedup = DayDedup::new();
        append_block(dir.path(), &block_at(9, 14, 41), &mut dedup).unwrap();
        append_block(dir.path(), &block_at(10, 0, 20), &mut dedup).unwrap();

        let path = file_path(dir.path(), NaiveDate::from_ymd_opt(2026, 8, 25).unwrap());
        let contents = fs::read_to_string(path).unwrap();

        assert_eq!(contents.matches("read the issue").count(), 1);
        assert_eq!(contents.matches("## ").count(), 2, "both headings kept");
    }

    #[test]
    fn a_fresh_dedup_is_seeded_from_the_existing_day_file() {
        let dir = tempdir().unwrap();
        append_block(dir.path(), &block_at(9, 14, 41), &mut DayDedup::new()).unwrap();
        // Simulates a restart: new dedup, same folder, same day.
        append_block(dir.path(), &block_at(10, 0, 20), &mut DayDedup::new()).unwrap();

        let path = file_path(dir.path(), NaiveDate::from_ymd_opt(2026, 8, 25).unwrap());
        let contents = fs::read_to_string(path).unwrap();
        assert_eq!(contents.matches("read the issue").count(), 1);
    }

    #[test]
    fn novel_lines_still_write_alongside_repeated_ones() {
        let dir = tempdir().unwrap();
        let mut dedup = DayDedup::new();
        append_block(dir.path(), &block_at(9, 14, 41), &mut dedup).unwrap();
        let mut second = block_at(10, 0, 20);
        second.lines = vec![
            "read the issue".to_string(),
            "drafted the reply".to_string(),
        ];
        append_block(dir.path(), &second, &mut dedup).unwrap();

        let path = file_path(dir.path(), NaiveDate::from_ymd_opt(2026, 8, 25).unwrap());
        let contents = fs::read_to_string(path).unwrap();
        assert_eq!(contents.matches("read the issue").count(), 1);
        assert_eq!(contents.matches("drafted the reply").count(), 1);
    }

    #[test]
    fn deleting_the_day_file_resets_the_dedup() {
        let dir = tempdir().unwrap();
        let mut dedup = DayDedup::new();
        append_block(dir.path(), &block_at(9, 14, 41), &mut dedup).unwrap();

        let path = file_path(dir.path(), NaiveDate::from_ymd_opt(2026, 8, 25).unwrap());
        fs::remove_file(&path).unwrap();

        append_block(dir.path(), &block_at(10, 0, 20), &mut dedup).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(
            contents.contains("read the issue"),
            "a fresh file gets the lines again, not bare headings"
        );
        assert!(contents.starts_with("---\ndate: 2026-08-25\n"));
    }

    #[test]
    fn reset_forgets_seen_lines() {
        let dir_a = tempdir().unwrap();
        let dir_b = tempdir().unwrap();
        let mut dedup = DayDedup::new();
        append_block(dir_a.path(), &block_at(9, 14, 41), &mut dedup).unwrap();
        dedup.reset();
        append_block(dir_b.path(), &block_at(10, 0, 20), &mut dedup).unwrap();

        let path = file_path(dir_b.path(), NaiveDate::from_ymd_opt(2026, 8, 25).unwrap());
        let contents = fs::read_to_string(path).unwrap();
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
        append_block(dir.path(), &first, &mut dedup).unwrap();

        let mut second = block_at(10, 0, 20);
        second.lines = vec![tweet_v2];
        append_block(dir.path(), &second, &mut dedup).unwrap();

        let path = file_path(dir.path(), NaiveDate::from_ymd_opt(2026, 8, 25).unwrap());
        let contents = fs::read_to_string(path).unwrap();
        assert!(contents.contains(&tweet_v1));
        assert!(!contents.contains("6 hours ago"), "the re-capture is dropped");
    }

    #[test]
    fn writes_agents_md_into_the_folder_once() {
        let dir = tempdir().unwrap();
        append_block(dir.path(), &block_at(9, 14, 41), &mut DayDedup::new()).unwrap();
        let agents = dir.path().join("AGENTS.md");
        assert!(agents.exists());

        fs::write(&agents, "user edited").unwrap();
        append_block(dir.path(), &block_at(10, 0, 20), &mut DayDedup::new()).unwrap();
        assert_eq!(fs::read_to_string(&agents).unwrap(), "user edited");
    }
}
