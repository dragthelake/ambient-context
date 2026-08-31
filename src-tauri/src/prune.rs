use regex::Regex;
use std::sync::OnceLock;

/// Characters that render as nothing but defeat emptiness checks. Today's
/// capture had hundreds of lines that were only these.
const ZERO_WIDTH: &[char] = &['\u{200b}', '\u{200c}', '\u{200d}', '\u{2060}', '\u{feff}'];

/// A short fragment is usually a UI label ("Shell", "Cancel"), but short
/// identifiers are the most valuable lines in the file. Anything carrying
/// a digit (versions, tickets, dates, "Task 4"), a URL, a path or an email
/// stays regardless of length.
fn is_high_value(line: &str) -> bool {
    static EMAIL: OnceLock<Regex> = OnceLock::new();
    let email = EMAIL.get_or_init(|| Regex::new(r"\b\S+@\S+\.\S+\b").unwrap());
    line.contains("://")
        || line.starts_with('/')
        || line.starts_with("~/")
        || line.chars().any(|c| c.is_ascii_digit())
        || email.is_match(line)
}

/// Bare counters and social chrome: view counts, vote counts, "n minutes
/// ago", media player positions. These change on every read, so they defeat
/// the day dedup while carrying nothing, and they were 22% of all captured
/// lines in a measured real day.
fn is_metric(line: &str) -> bool {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        vec![
            // digits and punctuation only: "440", "6:03", "12,211", "50%"
            Regex::new(r"^[\d\s.,:%+\-/]+$").unwrap(),
            // abbreviated counts: "42k", "1.2M"
            Regex::new(r"(?i)^[\d.,]+\s*[kmb]$").unwrap(),
            // labelled counts: "86 views", "440 points", "1,149 words"
            Regex::new(r"(?i)^[\d.,]+[kmb]?\s+(views?|points?|comments?|likes?|replies|followers?|words?|characters?|backlinks?|upvotes?|stars?|reposts?|quotes?|bookmarks?)$").unwrap(),
            // relative timestamps: "8 minutes ago"
            Regex::new(r"(?i)^\d+\s+(minutes?|hours?|seconds?|days?|weeks?|months?|years?)\s+ago$").unwrap(),
            // bare durations: "50m", "9h", "3 mins"
            Regex::new(r"(?i)^\d+\s?(min|mins|sec|secs|hrs?|h|m|s)$").unwrap(),
            // media player positions: "1 Minutes 43 Seconds of 10 Minutes 31 Seconds"
            Regex::new(r"(?i)^(\d+\s+(hours?|minutes?|seconds?)\s*)+of\s+(\d+\s+(hours?|minutes?|seconds?)\s*)+$").unwrap(),
        ]
    });
    patterns.iter().any(|p| p.is_match(line))
}

/// Site navigation bars read as pipe-separated menus: five or more short
/// segments is chrome, not content.
fn is_pipe_menu(line: &str) -> bool {
    let segments: Vec<&str> = line.split('|').collect();
    segments.len() >= 5 && segments.iter().all(|s| s.split_whitespace().count() <= 3)
}

/// The line with every digit run replaced by '#'. Two captures of the same
/// text that differ only in ticking numbers share a skeleton.
pub fn skeleton(line: &str) -> String {
    static DIGITS: OnceLock<Regex> = OnceLock::new();
    let digits = DIGITS.get_or_init(|| Regex::new(r"\d+").unwrap());
    digits.replace_all(line, "#").into_owned()
}

/// Mostly numbers and separators: a counter or timer rather than prose.
pub fn is_digit_heavy(line: &str) -> bool {
    let numeric = line
        .chars()
        .filter(|c| c.is_ascii_digit() || matches!(c, ':' | '.' | ',' | '%'))
        .count();
    numeric as f64 / line.len().max(1) as f64 > 0.25
}

/// Long enough and digit-varied enough that a repeat with different numbers
/// is a re-capture, not new content: tweets with ticking "ago" counters,
/// story rows with vote counts.
pub fn is_skeleton_dedupable(line: &str) -> bool {
    static DIGITS: OnceLock<Regex> = OnceLock::new();
    let digits = DIGITS.get_or_init(|| Regex::new(r"\d+").unwrap());
    line.len() > 40 && digits.find_iter(line).count() >= 2
}

/// Cleans one captured line. `None` means the line carries no information
/// worth a token: invisible characters, pure decoration, bare counters,
/// navigation chrome, or a fragment too short to mean anything on its own.
pub fn normalise_line(line: &str) -> Option<String> {
    let cleaned: String = line
        .chars()
        .filter(|c| !ZERO_WIDTH.contains(c))
        // Non-breaking space variants make visually identical lines hash
        // differently, which silently defeats the dedup.
        .map(|c| match c {
            '\u{a0}' | '\u{202f}' | '\u{2009}' | '\u{2007}' => ' ',
            other => other,
        })
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }
    if !collapsed.chars().any(|c| c.is_alphanumeric()) {
        return None;
    }
    if is_metric(&collapsed) || is_pipe_menu(&collapsed) {
        return None;
    }
    if collapsed.split_whitespace().count() < 4 && !is_high_value(&collapsed) {
        return None;
    }
    Some(collapsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_zero_width_only_lines() {
        assert_eq!(normalise_line("\u{200b}"), None);
        assert_eq!(normalise_line("\u{200b} \u{feff}"), None);
    }

    #[test]
    fn strips_zero_width_characters_inside_kept_lines() {
        let out = normalise_line("agreed to ship the\u{200b} notch state").unwrap();
        assert_eq!(out, "agreed to ship the notch state");
    }

    #[test]
    fn drops_decoration_only_lines() {
        assert_eq!(normalise_line("-"), None);
        assert_eq!(normalise_line("···"), None);
        assert_eq!(normalise_line("\u{2500}\u{2500}\u{2500}"), None);
    }

    #[test]
    fn drops_short_ui_labels() {
        assert_eq!(normalise_line("Shell"), None);
        assert_eq!(normalise_line("Open File"), None);
        assert_eq!(normalise_line("Reply All Forward"), None);
    }

    #[test]
    fn keeps_ordinary_sentences() {
        let line = "Agreed to ship the notch widen state on Thursday";
        assert_eq!(normalise_line(line).as_deref(), Some(line));
    }

    #[test]
    fn keeps_short_identifiers() {
        assert!(normalise_line("YN-102").is_some());
        assert!(normalise_line("v0.1.0").is_some());
        assert!(normalise_line("Task 4: Redaction").is_some());
        assert!(normalise_line("https://v2.tauri.app/").is_some());
        assert!(normalise_line("/Users/x/report.pdf").is_some());
        assert!(normalise_line("~/Sites/ambient-context").is_some());
        assert!(normalise_line("cam@example.com").is_some());
    }

    #[test]
    fn trims_whitespace() {
        assert_eq!(
            normalise_line("  padded but a real sentence here  ").as_deref(),
            Some("padded but a real sentence here")
        );
    }

    #[test]
    fn drops_bare_counters_and_social_chrome() {
        for junk in [
            "440",
            "6:03",
            "12,211",
            "42k",
            "1.2M",
            "86 views",
            "440 points",
            "38 comments",
            "1,149 words",
            "8 minutes ago",
            "50m",
            "9h",
            "1 Minutes 43 Seconds of 10 Minutes 31 Seconds",
        ] {
            assert_eq!(normalise_line(junk), None, "should drop {junk:?}");
        }
    }

    #[test]
    fn keeps_identifiers_that_metric_patterns_must_not_eat() {
        assert!(normalise_line("YN-102").is_some());
        assert!(normalise_line("v0.1.0").is_some());
        assert!(normalise_line("Task 4: Redaction").is_some());
        assert!(normalise_line("2026-08-25.md").is_some());
    }

    #[test]
    fn drops_pipe_menus_but_not_shell_pipelines() {
        assert_eq!(
            normalise_line("new | threads | past | comments | ask | show | jobs"),
            None
        );
        assert!(normalise_line("cat access.log | grep 500 | sort | uniq -c").is_some());
    }

    #[test]
    fn normalises_non_breaking_spaces_so_dedup_can_match() {
        assert_eq!(
            normalise_line("the same\u{a0}line as before, honestly").as_deref(),
            Some("the same line as before, honestly")
        );
    }

    #[test]
    fn skeleton_replaces_digit_runs() {
        assert_eq!(
            skeleton("440 points by x 9 hours ago"),
            "# points by x # hours ago"
        );
    }

    #[test]
    fn skeleton_dedupable_needs_length_and_two_digit_runs() {
        assert!(is_skeleton_dedupable(
            "Dan Verified account @dan 5 hours ago so it looks like 3 things broke"
        ));
        assert!(!is_skeleton_dedupable(
            "Round 2: the incremental loop, same day"
        ));
        assert!(!is_skeleton_dedupable("9:41 and 10:05"));
    }
}
