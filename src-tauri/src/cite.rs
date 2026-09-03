//! Time-range citations, shared by the ingest and summary validators. Both
//! ask the same question of a model's output: does this range point at a
//! block the day file actually recorded?

use regex::Regex;
use std::sync::OnceLock;

/// `09:14-09:41` or `09:14–09:41`. Both dashes appear in practice: the day
/// file's own headings use an en dash and models copy either.
fn range() -> &'static Regex {
    static RANGE: OnceLock<Regex> = OnceLock::new();
    RANGE.get_or_init(|| {
        Regex::new(r"\b(\d{1,2}):(\d{2})\s*[-\x{2013}]\s*(\d{1,2}):(\d{2})\b").unwrap()
    })
}

pub fn has_citation(text: &str) -> bool {
    range().is_match(text)
}

/// A block running past midnight is recorded with an end minute beyond
/// 24:00, so a citation after midnight has to be tried in both frames.
pub fn inside(minute: u32, spans: &[(u32, u32)]) -> bool {
    spans.iter().any(|(s, e)| minute >= *s && minute <= *e)
        || spans
            .iter()
            .any(|(s, e)| minute + 24 * 60 >= *s && minute + 24 * 60 <= *e)
}

/// Every range in `text` must fall inside a captured block. A citation the
/// day file cannot account for is worse than no citation: it reads as
/// evidence and is not. Returns the first offending range.
pub fn citation_in_spans(text: &str, spans: &[(u32, u32)]) -> Result<(), String> {
    for caps in range().captures_iter(text) {
        let n = |i: usize| caps[i].parse::<u32>().unwrap_or(0);
        let (start, end) = (n(1) * 60 + n(2), n(3) * 60 + n(4));
        if !inside(start, spans) || !inside(end, spans) {
            return Err(caps[0].to_string());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans() -> Vec<(u32, u32)> {
        vec![(540, 660), (720, 750)]
    }

    #[test]
    fn a_citation_inside_a_block_is_accepted_with_either_dash() {
        for sep in ["-", "\u{2013}", " - "] {
            assert!(citation_in_spans(&format!("did a thing 09:30{sep}10:00"), &spans()).is_ok());
        }
    }

    #[test]
    fn a_citation_between_blocks_is_named_back() {
        assert_eq!(
            citation_in_spans("wrote it up 11:10-11:40", &spans()).unwrap_err(),
            "11:10-11:40"
        );
    }

    #[test]
    fn a_block_running_past_midnight_covers_the_early_hours() {
        assert!(inside(30, &[(1380, 1470)]));
    }

    #[test]
    fn text_with_no_ranges_has_no_citation_and_nothing_to_check() {
        assert!(!has_citation("no ranges at all"));
        assert!(citation_in_spans("no ranges at all", &spans()).is_ok());
    }
}
