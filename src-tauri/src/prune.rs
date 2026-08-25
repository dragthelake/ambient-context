use regex::Regex;
use std::sync::OnceLock;

/// Characters that render as nothing but defeat emptiness checks. Today's
/// capture had hundreds of lines that were only these.
const ZERO_WIDTH: &[char] = &[
    '\u{200b}', '\u{200c}', '\u{200d}', '\u{2060}', '\u{feff}',
];

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

/// Cleans one captured line. `None` means the line carries no information
/// worth a token: invisible characters, pure decoration, or a fragment too
/// short to mean anything on its own.
pub fn normalise_line(line: &str) -> Option<String> {
    let cleaned: String = line.chars().filter(|c| !ZERO_WIDTH.contains(c)).collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return None;
    }
    if !trimmed.chars().any(|c| c.is_alphanumeric()) {
        return None;
    }
    if trimmed.split_whitespace().count() < 4 && !is_high_value(trimmed) {
        return None;
    }
    Some(trimmed.to_string())
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
}
