use crate::rules::{self, Rules};

/// Where a finished block's body goes. Decided at block close because a
/// browser block's URL often arrives a few polls after the block opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    App,
    Website,
    Message,
}

impl Kind {
    pub fn routed_name(self) -> Option<&'static str> {
        match self {
            Kind::App => None,
            Kind::Website => Some("websites"),
            Kind::Message => Some("messages"),
        }
    }
}

/// Applications whose windows are message surfaces, matched as a
/// case-insensitive substring of the application name.
pub const MESSAGE_APPS: &[&str] = &[
    "Mail", "Slack", "Discord", "Messages", "Linear", "Telegram", "WhatsApp",
];

/// Web addresses that are message surfaces: `host/path-prefix`, where a
/// `*` stands for one path segment. Matched against `rules::domain_of`
/// (so `www.` is already dropped) plus the URL path.
pub const MESSAGE_URLS: &[&str] = &[
    "mail.google.com",
    "outlook.live.com",
    "outlook.office.com",
    "github.com/*/*/pull/",
    "github.com/notifications",
    "linear.app/*/inbox",
    "x.com/messages",
    "x.com/notifications",
    "reddit.com/message",
    "linkedin.com/messaging",
    "discord.com/channels",
    "app.slack.com",
];

/// Applications that are browsers. A block from one of these with no URL
/// is still a page visit, not an app body.
pub const BROWSERS: &[&str] = &[
    "Safari", "Chrome", "Chromium", "Arc", "Firefox", "Brave", "Edge", "Dia", "Zen", "Vivaldi",
    "Opera",
];

/// Whether an application name carries `name` as a whole word. A substring
/// test would make "Elmedia Player" a browser (Dia) and "Linearity Curve" a
/// message surface (Linear), and a misrouted browser block loses its body.
fn names_match(app: &str, name: &str) -> bool {
    let wanted = name.to_lowercase();
    app.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word == wanted)
}

/// The path of a URL, `/` when there is none.
fn path_of(url: &str) -> String {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    match after_scheme.find('/') {
        Some(index) => after_scheme[index..]
            .split(['?', '#'])
            .next()
            .unwrap_or("/")
            .to_string(),
        None => "/".to_string(),
    }
}

/// `pattern` is `host` or `host/seg/seg/`, `*` matching one segment. The
/// pattern's path is a prefix of the URL's path on segment boundaries.
pub(crate) fn url_matches(pattern: &str, host: &str, path: &str) -> bool {
    let (pattern_host, pattern_path) = match pattern.split_once('/') {
        Some((h, p)) => (h, p),
        None => (pattern, ""),
    };
    if host != pattern_host && !host.ends_with(&format!(".{pattern_host}")) {
        return false;
    }
    let wanted: Vec<&str> = pattern_path.split('/').filter(|s| !s.is_empty()).collect();
    let actual: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if wanted.len() > actual.len() {
        return false;
    }
    wanted
        .iter()
        .zip(actual.iter())
        .all(|(w, a)| *w == "*" || w == a)
}

fn is_message_url(url: &str) -> bool {
    let Some(host) = rules::domain_of(url) else {
        return false;
    };
    let path = path_of(url);
    MESSAGE_URLS
        .iter()
        .any(|pattern| url_matches(pattern, &host, &path))
}

fn is_http(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

pub fn is_browser(app: &str) -> bool {
    BROWSERS.iter().any(|b| names_match(app, b))
}

/// Precedence: user route rule, built-in message table, http(s) means
/// website, browser without URL means website, otherwise app.
pub fn kind(rules: &Rules, app: &str, title: Option<&str>, url: Option<&str>) -> Kind {
    if rules::decide(rules, app, title, url) == rules::Decision::RouteMessages {
        return Kind::Message;
    }
    if MESSAGE_APPS.iter().any(|m| names_match(app, m)) {
        return Kind::Message;
    }
    if let Some(url) = url {
        if is_message_url(url) {
            return Kind::Message;
        }
        if is_http(url) {
            return Kind::Website;
        }
        return Kind::App;
    }
    if is_browser(app) {
        return Kind::Website;
    }
    Kind::App
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Action, Rule, Rules, Target};

    fn no_rules() -> Rules {
        Rules::default()
    }

    #[test]
    fn http_url_is_a_website() {
        assert_eq!(
            kind(
                &no_rules(),
                "Arc",
                Some("Tauri"),
                Some("https://v2.tauri.app/")
            ),
            Kind::Website
        );
        assert_eq!(
            kind(&no_rules(), "Safari", None, Some("http://localhost:1420/")),
            Kind::Website
        );
    }

    #[test]
    fn non_http_schemes_are_apps() {
        for url in [
            "app://obsidian.md/index.html",
            "file:///Applications/Claude.app/x.html",
            "x-webdoc://ABC",
            "tauri://localhost",
            "about:blank",
        ] {
            assert_eq!(
                kind(&no_rules(), "Obsidian", None, Some(url)),
                Kind::App,
                "{url}"
            );
        }
    }

    #[test]
    fn no_url_is_an_app_unless_the_app_is_a_browser() {
        assert_eq!(kind(&no_rules(), "Zed", Some("writer.rs"), None), Kind::App);
        for browser in BROWSERS {
            assert_eq!(
                kind(&no_rules(), browser, Some("Some page"), None),
                Kind::Website,
                "{browser}"
            );
        }
    }

    #[test]
    fn app_names_match_whole_words_not_substrings() {
        assert_eq!(
            kind(&no_rules(), "Elmedia Player", Some("clip.mp4"), None),
            Kind::App
        );
        assert_eq!(kind(&no_rules(), "Linearity Curve", None, None), Kind::App);
        assert_eq!(
            kind(&no_rules(), "Google Chrome", Some("page"), None),
            Kind::Website
        );
        assert_eq!(
            kind(&no_rules(), "Microsoft Edge", Some("page"), None),
            Kind::Website
        );
        assert_eq!(
            kind(&no_rules(), "Brave Browser", Some("page"), None),
            Kind::Website
        );
    }

    #[test]
    fn every_built_in_message_app_routes_to_messages() {
        for app in MESSAGE_APPS {
            assert_eq!(kind(&no_rules(), app, None, None), Kind::Message, "{app}");
        }
    }

    #[test]
    fn built_in_message_urls_route_to_messages_and_neighbours_do_not() {
        let cases = [
            (
                "https://github.com/dragthelake/ambient-context/pull/12",
                Kind::Message,
            ),
            (
                "https://github.com/dragthelake/ambient-context",
                Kind::Website,
            ),
            (
                "https://github.com/notifications?query=is%3Aunread",
                Kind::Message,
            ),
            ("https://linear.app/empty/inbox/YN-102", Kind::Message),
            ("https://linear.app/empty/issue/YN-102", Kind::Website),
            ("https://x.com/messages/123", Kind::Message),
            ("https://x.com/notifications", Kind::Message),
            ("https://x.com/home", Kind::Website),
            ("https://mail.google.com/mail/u/0/#inbox", Kind::Message),
            ("https://www.reddit.com/message/inbox", Kind::Message),
            ("https://old.reddit.com/r/rust/", Kind::Website),
            ("https://app.slack.com/client/T1/C1", Kind::Message),
            ("https://discord.com/channels/1/2", Kind::Message),
            ("https://www.linkedin.com/messaging/", Kind::Message),
            ("https://outlook.office.com/mail/", Kind::Message),
        ];
        for (url, expected) in cases {
            assert_eq!(kind(&no_rules(), "Arc", None, Some(url)), expected, "{url}");
        }
    }

    #[test]
    fn a_user_route_rule_beats_the_website_default() {
        let mut rules = Rules::default();
        rules.rules.push(Rule {
            id: "r1".into(),
            target: Target::Website("basecamp.com".into()),
            action: Action::RouteMessages,
            note: None,
        });
        assert_eq!(
            kind(&rules, "Arc", None, Some("https://3.basecamp.com/x")),
            Kind::Message
        );
    }

    #[test]
    fn a_narrower_headings_only_rule_beats_a_broader_route_rule() {
        let mut rules = Rules::default();
        rules.rules.push(Rule {
            id: "r1".into(),
            target: Target::App("Slack".into()),
            action: Action::RouteMessages,
            note: None,
        });
        rules.rules.push(Rule {
            id: "r2".into(),
            target: Target::Title("#random".into()),
            action: Action::HeadingsOnly,
            note: None,
        });
        // The built-in table still says Slack is a message surface; the
        // block is a headings-only message, which the writer handles.
        assert_eq!(kind(&rules, "Slack", Some("#random"), None), Kind::Message);
    }

    #[test]
    fn url_pattern_matching_handles_wildcards_and_www() {
        assert!(url_matches(
            "github.com/*/*/pull/",
            "github.com",
            "/a/b/pull/7"
        ));
        assert!(!url_matches(
            "github.com/*/*/pull/",
            "github.com",
            "/a/b/issues/7"
        ));
        assert!(url_matches("x.com/messages", "x.com", "/messages"));
        assert!(url_matches("x.com/messages", "x.com", "/messages/42"));
        assert!(!url_matches("x.com/messages", "x.com", "/messagesboard"));
        assert!(url_matches(
            "mail.google.com",
            "mail.google.com",
            "/mail/u/0/"
        ));
    }
}
