use crate::reader::Snapshot;
use regex::Regex;
use std::sync::OnceLock;

/// Applications whose contents are never read at all. Matched
/// case-insensitively against the application's localised name, as a
/// substring, so that "1Password 8" and "1Password" both match.
pub const EXCLUDED_APPS: &[&str] = &[
    "1password",
    "bitwarden",
    "dashlane",
    "enpass",
    "keepassxc",
    "keychain access",
    "lastpass",
    "nordpass",
    "proton pass",
    "strongbox",
];

pub fn is_excluded_app(app: &str) -> bool {
    let lower = app.to_lowercase();
    EXCLUDED_APPS
        .iter()
        .any(|excluded| lower.contains(excluded))
}

/// The app's own process. Its window shows settings text and the
/// summaries it wrote, which recorded 165 KB in one measured day and fed
/// the summary back into itself.
pub fn is_own_app(app: &str) -> bool {
    let lower = app.to_lowercase();
    lower == "ambient-context" || lower == "ambient context"
}

/// Window-title markers for private browsing in the major browsers. The
/// browser is the redaction layer's largest blind spot: banking and health
/// happen inside Safari and Chrome, which can never be on the app exclusion
/// list, and the title is the only per-site signal available. A private
/// window is the user saying "not this", so the whole snapshot is dropped.
pub const PRIVATE_WINDOW_MARKERS: &[&str] = &["private browsing", "incognito", "inprivate"];

pub fn is_private_window(title: &str) -> bool {
    let lower = title.to_lowercase();
    PRIVATE_WINDOW_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
}

fn patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // AWS access key id
            Regex::new(r"\bAKIA[0-9A-Z]{16}\b").unwrap(),
            // Provider-style secret keys: sk-..., pk_live_..., rk_...
            Regex::new(r"\b(?:sk|pk|rk)[-_][A-Za-z0-9_\-]{16,}\b").unwrap(),
            // Bearer tokens
            Regex::new(r"(?i)\bbearer\s+[A-Za-z0-9._\-]{16,}").unwrap(),
            // Labelled secrets: api_key = ..., token: ..., password=...
            Regex::new(r"(?i)\b(?:api[_-]?key|secret|token|password|passwd)\b\s*[:=]\s*\S+")
                .unwrap(),
            // Card-shaped digit runs, 13 to 19 digits with optional separators
            Regex::new(r"\b(?:\d[ -]?){13,19}\b").unwrap(),
        ]
    })
}

#[cfg(test)]
pub fn redact_line(line: &str) -> String {
    redact_line_with(line, &[])
}

/// The built-in patterns, then the user's own. A user pattern is a plain
/// regex; anything that does not compile is dropped, because a typo in
/// settings must never stop capture.
pub fn redact_line_with(line: &str, extra: &[Regex]) -> String {
    let mut out = line.to_string();
    for pattern in patterns().iter().chain(extra.iter()) {
        out = pattern.replace_all(&out, "[redacted]").into_owned();
    }
    out
}

pub fn compile_extra(patterns: &[String]) -> Vec<Regex> {
    patterns
        .iter()
        .filter(|p| !p.trim().is_empty())
        .filter_map(|p| match Regex::new(p) {
            Ok(compiled) => Some(compiled),
            Err(e) => {
                eprintln!("[redact] ignoring invalid pattern {p:?}: {e}");
                None
            }
        })
        .collect()
}

/// Returns `None` when the whole snapshot is dropped: its application is
/// excluded, its window is a private browsing window, or a user rule says
/// exclude. Otherwise returns the snapshot with every line redacted, and
/// `headings_only` set when a rule says record the heading only.
pub fn redact_snapshot(
    snapshot: Snapshot,
    rules: &crate::rules::Rules,
    extra: &[Regex],
) -> Option<Snapshot> {
    if is_excluded_app(&snapshot.app) {
        return None;
    }
    if snapshot
        .window_title
        .as_deref()
        .is_some_and(is_private_window)
    {
        return None;
    }
    let decision = crate::rules::decide(
        rules,
        &snapshot.app,
        snapshot.window_title.as_deref(),
        snapshot.url.as_deref(),
    );
    if decision == crate::rules::Decision::Exclude {
        return None;
    }
    let own_app = is_own_app(&snapshot.app);
    Some(Snapshot {
        app: snapshot.app,
        window_title: snapshot.window_title.map(|t| redact_line_with(&t, extra)),
        document: snapshot.document.map(|d| redact_line_with(&d, extra)),
        url: snapshot.url.map(|u| redact_line_with(&u, extra)),
        text: snapshot
            .text
            .iter()
            .map(|l| redact_line_with(l, extra))
            .collect(),
        headings_only: decision == crate::rules::Decision::HeadingsOnly || own_app,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excludes_password_managers_case_insensitively() {
        assert!(is_excluded_app("1Password"));
        assert!(is_excluded_app("1Password 8"));
        assert!(is_excluded_app("BITWARDEN"));
        assert!(is_excluded_app("Keychain Access"));
    }

    #[test]
    fn does_not_exclude_ordinary_applications() {
        assert!(!is_excluded_app("Linear"));
        assert!(!is_excluded_app("Slack"));
        assert!(!is_excluded_app("Safari"));
    }

    #[test]
    fn the_apps_own_window_is_headings_only() {
        use crate::reader::Snapshot;
        let snap = Snapshot {
            app: "Ambient Context".into(),
            text: vec!["Volume 55 %".into()],
            ..Default::default()
        };
        let out = redact_snapshot(snap, &crate::rules::Rules::default(), &[]).unwrap();
        assert!(out.headings_only);
    }

    #[test]
    fn redacts_an_aws_key() {
        let out = redact_line("key is AKIAIOSFODNN7EXAMPLE ok");
        assert!(!out.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(out.contains("[redacted]"));
    }

    #[test]
    fn redacts_provider_secret_keys() {
        let out = redact_line("sk-abcdefghijklmnopqrstuvwx");
        assert_eq!(out, "[redacted]");
    }

    #[test]
    fn redacts_labelled_secrets() {
        let out = redact_line("api_key = hunter2hunter2hunter2");
        assert!(!out.contains("hunter2hunter2hunter2"));
    }

    #[test]
    fn redacts_card_shaped_numbers() {
        let out = redact_line("4111 1111 1111 1111");
        assert!(!out.contains("4111"));
    }

    #[test]
    fn leaves_ordinary_prose_alone() {
        let line = "Agreed to ship the notch widen state on Thursday";
        assert_eq!(redact_line(line), line);
    }

    #[test]
    fn recognises_private_windows_across_browsers() {
        assert!(is_private_window("Login - Private Browsing"));
        assert!(is_private_window("New Incognito tab - Google Chrome"));
        assert!(is_private_window("Bing - [InPrivate] - Microsoft Edge"));
    }

    #[test]
    fn does_not_flag_ordinary_windows_as_private() {
        assert!(!is_private_window("YN-102 Proposal protocol"));
        assert!(!is_private_window("#empty-build - Empty"));
    }

    #[test]
    fn drops_the_whole_snapshot_for_a_private_window() {
        let snapshot = Snapshot {
            app: "Safari".to_string(),
            window_title: Some("Bankwest - Private Browsing".to_string()),
            text: vec!["account balance".to_string()],
            ..Default::default()
        };
        assert!(redact_snapshot(snapshot, &Rules::default(), &[]).is_none());
    }

    #[test]
    fn drops_the_whole_snapshot_for_an_excluded_app() {
        let snapshot = Snapshot {
            app: "1Password".to_string(),
            window_title: Some("Vault".to_string()),
            text: vec!["secret".to_string()],
            ..Default::default()
        };
        assert!(redact_snapshot(snapshot, &Rules::default(), &[]).is_none());
    }

    #[test]
    fn redacts_the_window_title_too() {
        let snapshot = Snapshot {
            app: "Terminal".to_string(),
            window_title: Some("export TOKEN=abcdefghijklmnopqrst".to_string()),
            text: vec![],
            ..Default::default()
        };
        let out = redact_snapshot(snapshot, &Rules::default(), &[]).unwrap();
        assert!(!out.window_title.unwrap().contains("abcdefghijklmnopqrst"));
    }

    #[test]
    fn redacts_secrets_inside_urls() {
        let snapshot = Snapshot {
            app: "Safari".to_string(),
            window_title: Some("Dashboard".to_string()),
            url: Some("https://example.com/cb?token=abcdefghijklmnopqrst".to_string()),
            ..Default::default()
        };
        let out = redact_snapshot(snapshot, &Rules::default(), &[]).unwrap();
        assert!(!out.url.unwrap().contains("abcdefghijklmnopqrst"));
    }

    use crate::rules::{Action, Rule, Rules, Target};

    fn with(rule: Rule) -> Rules {
        let mut set = Rules::default();
        set.add(rule).unwrap();
        set
    }

    #[test]
    fn an_exclusion_rule_drops_the_snapshot() {
        let rules = with(Rule {
            id: "r1".to_string(),
            target: Target::App("Slack".to_string()),
            action: Action::Exclude,
            note: None,
        });
        let snapshot = Snapshot {
            app: "Slack".to_string(),
            window_title: Some("#empty-build".to_string()),
            text: vec!["standup notes".to_string()],
            ..Default::default()
        };
        assert!(redact_snapshot(snapshot, &rules, &[]).is_none());
    }

    #[test]
    fn a_headings_only_rule_marks_the_snapshot_and_keeps_its_text() {
        let rules = with(Rule {
            id: "r1".to_string(),
            target: Target::Website("news.ycombinator.com".to_string()),
            action: Action::HeadingsOnly,
            note: None,
        });
        let snapshot = Snapshot {
            app: "Safari".to_string(),
            window_title: Some("Hacker News".to_string()),
            url: Some("https://news.ycombinator.com/".to_string()),
            text: vec!["a story title".to_string()],
            ..Default::default()
        };
        let out = redact_snapshot(snapshot, &rules, &[]).unwrap();
        assert!(out.headings_only);
        assert_eq!(out.text, vec!["a story title".to_string()]);
    }

    #[test]
    fn an_unmatched_snapshot_is_not_headings_only() {
        let snapshot = Snapshot {
            app: "Linear".to_string(),
            window_title: Some("YN-102".to_string()),
            text: vec!["one".to_string()],
            ..Default::default()
        };
        let out = redact_snapshot(snapshot, &Rules::default(), &[]).unwrap();
        assert!(!out.headings_only);
    }

    #[test]
    fn user_patterns_redact_alongside_the_built_ins() {
        let extra = compile_extra(&["Project Kestrel".to_string()]);
        let out = redact_line_with("we shipped Project Kestrel today", &extra);
        assert_eq!(out, "we shipped [redacted] today");
    }

    #[test]
    fn an_invalid_user_pattern_is_skipped_rather_than_breaking_capture() {
        let extra = compile_extra(&["([unclosed".to_string(), "Kestrel".to_string()]);
        assert_eq!(extra.len(), 1);
        assert_eq!(redact_line_with("Kestrel", &extra), "[redacted]");
    }
}
