use crate::segment::Block;
use chrono::{Datelike, NaiveDate};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

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

pub fn render_block(block: &Block) -> String {
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
    for line in &block.lines {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Appends one block to the file for the block's own start date, creating the
/// folder and the file with frontmatter if they do not exist yet.
pub fn append_block(folder: &Path, block: &Block) -> std::io::Result<()> {
    fs::create_dir_all(folder)?;
    let date = block.start.date_naive();
    let path = file_path(folder, date);
    let is_new = !path.exists();

    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    if is_new {
        file.write_all(frontmatter(date).as_bytes())?;
    }
    file.write_all(render_block(block).as_bytes())?;
    Ok(())
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
            start: Local.with_ymd_and_hms(2026, 8, 25, hour, minute, 0).unwrap(),
            end: Local
                .with_ymd_and_hms(2026, 8, 25, hour, end_minute, 0)
                .unwrap(),
            lines: vec!["read the issue".to_string()],
        }
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
        let out = render_block(&block_at(9, 14, 41));
        assert!(out.contains("## 09:14\u{2013}09:41 \u{00b7} Linear \u{00b7} YN-102"));
        assert!(out.contains("read the issue"));
    }

    #[test]
    fn renders_without_a_title_when_there_is_none() {
        let mut block = block_at(9, 14, 41);
        block.title = None;
        let out = render_block(&block);
        assert!(out.contains("\u{00b7} Linear\n"));
    }

    #[test]
    fn creates_the_file_with_frontmatter_once() {
        let dir = tempdir().unwrap();
        append_block(dir.path(), &block_at(9, 14, 41)).unwrap();
        append_block(dir.path(), &block_at(10, 0, 20)).unwrap();

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
        append_block(&nested, &block_at(9, 14, 41)).unwrap();
        assert!(nested.exists());
    }
}
