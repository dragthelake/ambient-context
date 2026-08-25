use crate::reader::Snapshot;
use chrono::{DateTime, Duration, Local};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub app: String,
    pub title: Option<String>,
    pub document: Option<String>,
    pub url: Option<String>,
    pub start: DateTime<Local>,
    pub end: DateTime<Local>,
    pub lines: Vec<String>,
}

/// Set similarity over whitespace-split tokens. Returns 1.0 for two empty
/// inputs so that a pair of blank reads does not spuriously start a block.
pub fn jaccard(a: &[String], b: &[String]) -> f64 {
    let left: HashSet<&str> = a.iter().flat_map(|s| s.split_whitespace()).collect();
    let right: HashSet<&str> = b.iter().flat_map(|s| s.split_whitespace()).collect();
    if left.is_empty() && right.is_empty() {
        return 1.0;
    }
    let intersection = left.intersection(&right).count() as f64;
    let union = left.union(&right).count() as f64;
    if union == 0.0 {
        return 1.0;
    }
    intersection / union
}

struct OpenBlock {
    app: String,
    title: Option<String>,
    document: Option<String>,
    url: Option<String>,
    start: DateTime<Local>,
    end: DateTime<Local>,
    lines: Vec<String>,
    seen: HashSet<String>,
    // Skeletons of digit-heavy lines already in the block, so a timer or
    // counter ticking across polls lands once instead of once per tick.
    skeletons: HashSet<String>,
    last_text: Vec<String>,
}

fn admit(line: &str, seen: &mut HashSet<String>, skeletons: &mut HashSet<String>) -> bool {
    if seen.contains(line) {
        return false;
    }
    if crate::prune::is_digit_heavy(line) {
        let skeleton = crate::prune::skeleton(line);
        if !skeletons.insert(skeleton) {
            return false;
        }
    }
    seen.insert(line.to_string());
    true
}

pub struct Segmenter {
    open: Option<OpenBlock>,
    min_dwell: Duration,
    similarity_threshold: f64,
}

impl Segmenter {
    pub fn new(min_dwell_secs: i64, similarity_threshold: f64) -> Self {
        Segmenter {
            open: None,
            min_dwell: Duration::seconds(min_dwell_secs),
            similarity_threshold,
        }
    }

    /// Feeds one snapshot in. Returns a finished block when this snapshot
    /// closed the previous one and that block was long enough to keep.
    pub fn push(&mut self, snapshot: Snapshot, now: DateTime<Local>) -> Option<Block> {
        let starts_new = match &self.open {
            None => true,
            Some(open) => {
                open.app != snapshot.app
                    || open.title != snapshot.window_title
                    || jaccard(&open.last_text, &snapshot.text) < self.similarity_threshold
            }
        };

        if !starts_new {
            let open = self.open.as_mut().expect("checked above");
            open.end = now;
            // A reference can arrive late: the first reads of a freshly
            // enabled web area often lack the URL the later ones carry.
            if open.document.is_none() {
                open.document = snapshot.document;
            }
            if open.url.is_none() {
                open.url = snapshot.url;
            }
            for line in &snapshot.text {
                if admit(line, &mut open.seen, &mut open.skeletons) {
                    open.lines.push(line.clone());
                }
            }
            open.last_text = snapshot.text;
            return None;
        }

        let finished = self.close(now);
        let mut seen = HashSet::new();
        let mut skeletons = HashSet::new();
        let mut lines = Vec::new();
        for line in &snapshot.text {
            if admit(line, &mut seen, &mut skeletons) {
                lines.push(line.clone());
            }
        }
        self.open = Some(OpenBlock {
            app: snapshot.app,
            title: snapshot.window_title,
            document: snapshot.document,
            url: snapshot.url,
            start: now,
            end: now,
            lines,
            seen,
            skeletons,
            last_text: snapshot.text,
        });
        finished
    }

    /// Closes any open block, for shutdown or for switching capture off.
    pub fn flush(&mut self, now: DateTime<Local>) -> Option<Block> {
        self.close(now)
    }

    fn close(&mut self, now: DateTime<Local>) -> Option<Block> {
        let open = self.open.take()?;
        let end = if now > open.end { now } else { open.end };
        if end - open.start < self.min_dwell {
            return None;
        }
        if open.lines.is_empty() {
            return None;
        }
        Some(Block {
            app: open.app,
            title: open.title,
            document: open.document,
            url: open.url,
            start: open.start,
            end,
            lines: open.lines,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(second: i64) -> DateTime<Local> {
        Local.timestamp_opt(1_756_000_000 + second, 0).unwrap()
    }

    fn snap(app: &str, title: &str, text: &[&str]) -> Snapshot {
        Snapshot {
            app: app.to_string(),
            window_title: Some(title.to_string()),
            text: text.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn a_ticking_counter_lands_once_per_block() {
        let mut seg = Segmenter::new(0, 0.3);
        seg.push(snap("Spotify", "Now Playing", &["some song title words here", "1:41"]), at(0));
        seg.push(snap("Spotify", "Now Playing", &["some song title words here", "1:46"]), at(5));
        seg.push(snap("Spotify", "Now Playing", &["some song title words here", "1:51"]), at(10));
        let block = seg.flush(at(60)).unwrap();
        let timers: Vec<_> = block.lines.iter().filter(|l| l.contains(':')).collect();
        assert_eq!(timers, vec!["1:41"], "later ticks share the skeleton");
    }

    #[test]
    fn a_reference_arriving_late_is_kept_on_the_block() {
        let mut seg = Segmenter::new(0, 0.5);
        seg.push(snap("Safari", "Tauri docs", &["one two"]), at(0));
        let mut with_url = snap("Safari", "Tauri docs", &["one two"]);
        with_url.url = Some("https://v2.tauri.app/".to_string());
        seg.push(with_url, at(5));
        let block = seg.flush(at(60)).unwrap();
        assert_eq!(block.url.as_deref(), Some("https://v2.tauri.app/"));
    }

    #[test]
    fn jaccard_is_one_for_identical_input() {
        let a = vec!["hello world".to_string()];
        assert_eq!(jaccard(&a, &a), 1.0);
    }

    #[test]
    fn jaccard_is_zero_for_disjoint_input() {
        let a = vec!["alpha".to_string()];
        let b = vec!["beta".to_string()];
        assert_eq!(jaccard(&a, &b), 0.0);
    }

    #[test]
    fn identical_reads_do_not_start_a_new_block() {
        let mut seg = Segmenter::new(30, 0.5);
        assert!(seg
            .push(snap("Linear", "YN-102", &["one two"]), at(0))
            .is_none());
        assert!(seg
            .push(snap("Linear", "YN-102", &["one two"]), at(5))
            .is_none());
        assert!(seg
            .push(snap("Linear", "YN-102", &["one two"]), at(10))
            .is_none());
        let block = seg.flush(at(60)).unwrap();
        assert_eq!(block.lines, vec!["one two".to_string()]);
    }

    #[test]
    fn changing_application_closes_the_block() {
        let mut seg = Segmenter::new(0, 0.5);
        seg.push(snap("Linear", "YN-102", &["one"]), at(0));
        let finished = seg.push(snap("Slack", "#empty-build", &["two"]), at(60));
        let block = finished.expect("previous block should close");
        assert_eq!(block.app, "Linear");
        assert_eq!(block.start, at(0));
    }

    #[test]
    fn changing_window_title_closes_the_block() {
        let mut seg = Segmenter::new(0, 0.5);
        seg.push(snap("Linear", "YN-102", &["one"]), at(0));
        let finished = seg.push(snap("Linear", "YN-103", &["one"]), at(60));
        assert!(finished.is_some());
    }

    #[test]
    fn diverging_content_closes_the_block() {
        let mut seg = Segmenter::new(0, 0.5);
        seg.push(snap("Notes", "Scratch", &["alpha beta gamma"]), at(0));
        let finished = seg.push(snap("Notes", "Scratch", &["delta epsilon zeta"]), at(60));
        assert!(finished.is_some());
    }

    #[test]
    fn short_visits_are_discarded() {
        let mut seg = Segmenter::new(30, 0.5);
        seg.push(snap("Linear", "YN-102", &["one"]), at(0));
        let finished = seg.push(snap("Slack", "#empty-build", &["two"]), at(5));
        assert!(
            finished.is_none(),
            "5s visit is under the 30s dwell threshold"
        );
    }

    #[test]
    fn empty_blocks_are_discarded() {
        let mut seg = Segmenter::new(0, 0.5);
        seg.push(snap("Finder", "Desktop", &[]), at(0));
        assert!(seg.flush(at(120)).is_none());
    }

    #[test]
    fn new_lines_accumulate_without_duplicating() {
        let mut seg = Segmenter::new(0, 0.3);
        seg.push(snap("Notes", "Scratch", &["alpha", "beta"]), at(0));
        seg.push(snap("Notes", "Scratch", &["alpha", "beta", "gamma"]), at(5));
        let block = seg.flush(at(60)).unwrap();
        assert_eq!(
            block.lines,
            vec![
                "alpha".to_string(),
                "beta".to_string(),
                "gamma".to_string()
            ]
        );
    }
}
