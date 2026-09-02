# Days and Daily KB Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Capture writes `Days/YYYY-MM-DD/{apps,websites,messages}.md` instead of one day file; three ingest calls turn those into `KB/YYYY-MM-DD/`; the summary reads the KB and the timeline only.

**Architecture:** A new `route.rs` classifies a finished block at close and `writer.rs` fans it out to three append-only files. `days.rs` reads the folder layout and computes website totals at read time. A new `ingest.rs` splits, validates and atomically writes agent output per call; `jobs.rs` runs a three-call ingest loop then the summary. Prompts generalise to four ids. The Day view gains Raw tabs and a KB mode; MCP gains `read_kb` and `ingest_day`.

**Tech Stack:** Rust (chrono, serde, regex, sha2), Tauri 2, React 19, TypeScript, Vitest with jsdom, bundled prompts under `src-tauri/prompts/`.

**Spec:** `docs/superpowers/specs/2026-09-02-days-and-kb-design.md`

## Global Constraints

- Australian English in all prose, comments and UI copy. NEVER an em-dash (U+2014) anywhere, including commit messages; a git hook blocks them. Use a comma, colon, parentheses or two sentences. Block headings keep their en-dash (U+2013) between times, as `writer::render_block` already writes; in plan and test code write it as `\u{2013}`.
- `Days/` files are append-only and never rewritten. `KB/` and `Summaries/` are derived and regenerable.
- Flat `YYYY-MM-DD.md` files at the capture folder root are ignored by every reader. Never migrated, never deleted.
- Ledger field `engine` stays as-is on disk. New ledger actions: `ingest_messages`, `ingest_apps`, `ingest_websites`. Existing `summarise_day` keeps its name.
- Summary output validation (`summarise::validate`, `REQUIRED_HEADINGS`, `MAX_SUMMARY_LINES`) is unchanged. Only summary inputs change.
- No `@testing-library/jest-dom`. Plain vitest matchers only. Tauri is mocked through `src/test/tauri-mock.ts`; every test names the commands it expects, and an unnamed command throws.
- CI gates, run from the repo root before every commit: `cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && cd .. && npx tsc --noEmit && npx vitest run && npm run build`.
- Commit messages: imperative, no `Co-Authored-By`, no "Generated with" footer.
- Version ships as `0.2.0` (Task 14).

---

## File Structure

| File | Responsibility |
| --- | --- |
| `src-tauri/src/route.rs` | New. `Kind`, built-in message table, browser list, `kind()`. |
| `src-tauri/src/rules.rs` | `Action::RouteMessages`, `Decision::RouteMessages`, two new built-in rows. |
| `src-tauri/src/redact.rs` | `is_own_app`; own window forced to headings-only. |
| `src-tauri/src/writer.rs` | `DayFile`, `day_dir`, per-kind rendering, `append_block` fan-out, dedup seeded from two files. |
| `src-tauri/src/prune.rs` | `for_kind` with the Message filters. |
| `src-tauri/src/capture.rs` | Pass rules to the writer; `is_own_output` covers the new paths and titles. |
| `src-tauri/src/days.rs` | Folder scan, `read_day(file)`, `timeline`, `spans`, `website_totals`, `render_totals`, `parse_blocks` with `routed`. |
| `src-tauri/src/summarise.rs` | `list_captured` scans `Days/`; `build_prompt` takes timeline and KB. |
| `src-tauri/src/prompt.rs` | `PromptId`, four bundled prompts, per-prompt validation. |
| `src-tauri/prompts/day-context.md` | Rewritten input section, `{{TIMELINE}}` and `{{KB}}`. |
| `src-tauri/prompts/ingest-messages.md` | New bundled prompt. |
| `src-tauri/prompts/ingest-apps.md` | New bundled prompt. |
| `src-tauri/prompts/ingest-websites.md` | New bundled prompt. |
| `src-tauri/src/settings.rs` | `ingest_agent`, `ingest_max_chars`. |
| `src-tauri/src/ingest.rs` | New. `Call`, paths, split, validate, trim, atomic write, manifest, `needs_ingest`. |
| `src-tauri/src/jobs.rs` | `Pipeline`, `ingest_call`, `run_day_pipeline`, `JobKind`, step text, summary from KB. |
| `src-tauri/src/lib.rs` | Commands: `read_day(file)`, `read_day_blocks(file)`, `website_totals`, `read_kb`, `ingest_now`, prompt ids, editor targets. |
| `src-tauri/src/ipc.rs`, `control.rs`, `mcp/client.rs`, `mcp/tools.rs`, `mcp/files.rs` | `read_day` file arg, `read_kb`, `ingest_day`, search over three files. |
| `src-tauri/assets/AGENTS.md` | Rewritten for the new layout. |
| `docs/mcp.md`, `docs/handover.md`, `CHANGELOG.md` | Docs. |
| `src/lib/days.ts` | Types: `DayFile`, `UrlTotal`, `has_kb`, settings fields, `JobState.step`. |
| `src/components/RawPane.tsx` | Takes `file`. |
| `src/components/WebsitesPane.tsx` | New. Totals table. |
| `src/components/KbPane.tsx` | New. Six-file tabs. |
| `src/components/DayView.tsx`, `DayHeader.tsx` | Three modes, raw tabs, Ingest and Re-ingest, step text. |
| `src/components/PromptSettings.tsx` | Prompt selector. |
| `src/components/AgentTab.tsx` | Ingest agent picker, `ingest_max_chars`. |
| `src/components/RulesSettings.tsx`, `src/lib/rules.ts` | `route_messages` action. |

---

## PR 1: Capture into Days/

### Task 1: Routing module and the `route_messages` action

**Files:**
- Create: `src-tauri/src/route.rs`
- Modify: `src-tauri/src/rules.rs` (enum `Action` at line 27, enum `Decision` at line 276, `protection` at line 338, `built_ins()` at line 121)
- Modify: `src-tauri/src/redact.rs` (`redact_snapshot` at line 95)
- Modify: `src-tauri/src/lib.rs` (add `mod route;` beside the other `mod` lines)
- Modify: `src/lib/rules.ts`, `src/components/RulesSettings.tsx`, `src-tauri/src/mcp/tools.rs` (`rule_property` enum at line 44), `src-tauri/src/mcp/client.rs` (rule parse error text), `docs/mcp.md` (`add_rule` section)

**Interfaces:**
- Produces: `route::Kind { App, Website, Message }` with `routed_name(self) -> Option<&'static str>`, `route::kind(rules: &rules::Rules, app: &str, title: Option<&str>, url: Option<&str>) -> Kind`, `route::is_browser(app) -> bool`, `route::MESSAGE_APPS`, `route::MESSAGE_URLS`, `route::BROWSERS`, `rules::Action::RouteMessages`, `rules::Decision::RouteMessages`, `redact::is_own_app(app: &str) -> bool`.

- [ ] **Step 1: Add the action and decision variants in `rules.rs`**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Exclude,
    HeadingsOnly,
    Full,
    /// Record the body in messages.md rather than treating the block as a
    /// visit row or an app body.
    RouteMessages,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Exclude,
    HeadingsOnly,
    Full,
    RouteMessages,
}
```

In `protection`, add `Action::RouteMessages => 0` (as protective as Full). In `decide`, add `Some(Action::RouteMessages) => Decision::RouteMessages`.

Append two rows to `built_ins()`:

```rust
BuiltIn {
    id: "builtin:message-surfaces".to_string(),
    description: format!(
        "Bodies from message surfaces are recorded in messages.md. Applications: {}. Web addresses: {}.",
        crate::route::MESSAGE_APPS.join(", "),
        crate::route::MESSAGE_URLS.join(", ")
    ),
},
BuiltIn {
    id: "builtin:own-window".to_string(),
    description: "Ambient Context's own window is recorded as headings only.".to_string(),
},
```

- [ ] **Step 2: Write `route.rs` tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Action, Rule, Rules, Target};

    fn no_rules() -> Rules {
        Rules::default()
    }

    #[test]
    fn http_url_is_a_website() {
        assert_eq!(kind(&no_rules(), "Arc", Some("Tauri"), Some("https://v2.tauri.app/")), Kind::Website);
        assert_eq!(kind(&no_rules(), "Safari", None, Some("http://localhost:1420/")), Kind::Website);
    }

    #[test]
    fn non_http_schemes_are_apps() {
        for url in ["app://obsidian.md/index.html", "file:///Applications/Claude.app/x.html", "x-webdoc://ABC", "tauri://localhost", "about:blank"] {
            assert_eq!(kind(&no_rules(), "Obsidian", None, Some(url)), Kind::App, "{url}");
        }
    }

    #[test]
    fn no_url_is_an_app_unless_the_app_is_a_browser() {
        assert_eq!(kind(&no_rules(), "Zed", Some("writer.rs"), None), Kind::App);
        for browser in BROWSERS {
            assert_eq!(kind(&no_rules(), browser, Some("Some page"), None), Kind::Website, "{browser}");
        }
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
            ("https://github.com/dragthelake/ambient-context/pull/12", Kind::Message),
            ("https://github.com/dragthelake/ambient-context", Kind::Website),
            ("https://github.com/notifications?query=is%3Aunread", Kind::Message),
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
        assert_eq!(kind(&rules, "Arc", None, Some("https://3.basecamp.com/x")), Kind::Message);
    }

    #[test]
    fn a_narrower_headings_only_rule_beats_a_broader_route_rule() {
        let mut rules = Rules::default();
        rules.rules.push(Rule { id: "r1".into(), target: Target::App("Slack".into()), action: Action::RouteMessages, note: None });
        rules.rules.push(Rule { id: "r2".into(), target: Target::Title("#random".into()), action: Action::HeadingsOnly, note: None });
        // The built-in table still says Slack is a message surface; the
        // block is a headings-only message, which the writer handles.
        assert_eq!(kind(&rules, "Slack", Some("#random"), None), Kind::Message);
    }

    #[test]
    fn url_pattern_matching_handles_wildcards_and_www() {
        assert!(url_matches("github.com/*/*/pull/", "github.com", "/a/b/pull/7"));
        assert!(!url_matches("github.com/*/*/pull/", "github.com", "/a/b/issues/7"));
        assert!(url_matches("x.com/messages", "x.com", "/messages"));
        assert!(url_matches("x.com/messages", "x.com", "/messages/42"));
        assert!(!url_matches("x.com/messages", "x.com", "/messagesboard"));
        assert!(url_matches("mail.google.com", "mail.google.com", "/mail/u/0/"));
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd src-tauri && cargo test route::`
Expected: FAIL (module not found).

- [ ] **Step 4: Implement `route.rs`**

```rust
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
    "Safari", "Chrome", "Chromium", "Arc", "Firefox", "Brave", "Edge", "Dia", "Zen", "Vivaldi", "Opera",
];

fn contains_ci(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// The path of a URL, `/` when there is none.
fn path_of(url: &str) -> String {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    match after_scheme.find('/') {
        Some(index) => after_scheme[index..].split(['?', '#']).next().unwrap_or("/").to_string(),
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
    wanted.iter().zip(actual.iter()).all(|(w, a)| *w == "*" || w == a)
}

fn is_message_url(url: &str) -> bool {
    let Some(host) = rules::domain_of(url) else {
        return false;
    };
    let path = path_of(url);
    MESSAGE_URLS.iter().any(|pattern| url_matches(pattern, &host, &path))
}

fn is_http(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

pub fn is_browser(app: &str) -> bool {
    BROWSERS.iter().any(|b| contains_ci(app, b))
}

/// Precedence: user route rule, built-in message table, http(s) means
/// website, browser without URL means website, otherwise app.
pub fn kind(rules: &Rules, app: &str, title: Option<&str>, url: Option<&str>) -> Kind {
    if rules::decide(rules, app, title, url) == rules::Decision::RouteMessages {
        return Kind::Message;
    }
    if MESSAGE_APPS.iter().any(|m| contains_ci(app, m)) {
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
```

Add `mod route;` to `lib.rs`. `redact.rs` compares `decision == HeadingsOnly`, so a `RouteMessages` decision already yields `headings_only: false`.

- [ ] **Step 5: Own window is headings-only, in `redact.rs`**

```rust
/// The app's own process. Its window shows settings text and the
/// summaries it wrote, which recorded 165 KB in one measured day and fed
/// the summary back into itself.
pub fn is_own_app(app: &str) -> bool {
    let lower = app.to_lowercase();
    lower == "ambient-context" || lower == "ambient context"
}
```

In `redact_snapshot`, the last field becomes
`headings_only: decision == crate::rules::Decision::HeadingsOnly || is_own_app(&snapshot.app),`.

Test in `redact.rs`:

```rust
#[test]
fn the_apps_own_window_is_headings_only() {
    let snap = Snapshot { app: "Ambient Context".into(), text: vec!["Volume 55 %".into()], ..Default::default() };
    let out = redact_snapshot(snap, &crate::rules::Rules::default(), &[]).unwrap();
    assert!(out.headings_only);
}
```

- [ ] **Step 6: Frontend and MCP know the new action**

`src/lib/rules.ts`: `export type RuleAction = "exclude" | "headings_only" | "full" | "route_messages";`

`src/components/RulesSettings.tsx` `ACTION_LABELS`: add `route_messages: "Record in messages.md"`. In `builtInTitle`, add cases `"builtin:message-surfaces"` returning `"Message surfaces"` and `"builtin:own-window"` returning `"Own window"`.

`src-tauri/src/mcp/tools.rs` `rule_property`: enum becomes `["exclude", "headings_only", "full", "route_messages"]` and the description gains `", route_messages records the body in messages.md"`. `src-tauri/src/mcp/client.rs` rule parse error text: add `route_messages` to the listed actions. `docs/mcp.md` `add_rule`: add the same.

- [ ] **Step 7: Run tests**

Run: `cd src-tauri && cargo test route:: rules:: redact::`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/route.rs src-tauri/src/rules.rs src-tauri/src/redact.rs src-tauri/src/lib.rs src-tauri/src/mcp/tools.rs src-tauri/src/mcp/client.rs docs/mcp.md src/lib/rules.ts src/components/RulesSettings.tsx
git commit -m "Route finished blocks by kind and add the route_messages action"
```

---

### Task 2: Writer fans out to three day files

**Files:**
- Modify: `src-tauri/src/writer.rs` (everything except `ensure_agents_file`)
- Modify: `src-tauri/src/capture.rs` (four `append_block` call sites, `is_own_output`)
- Touch so the crate compiles: `src-tauri/src/days.rs`, `jobs.rs`, `lib.rs`, `mcp/files.rs` (see Step 7)

**Interfaces:**
- Consumes: `route::kind`, `route::Kind`, `rules::Rules`.
- Produces: `writer::DayFile { Apps, Websites, Messages }` with `all() -> [DayFile; 3]`, `file_name(self) -> &'static str`, `kind_name(self) -> &'static str`, `from_name(&str) -> Option<DayFile>`, `path(self, folder, date) -> PathBuf`; `writer::days_dir(folder) -> PathBuf`; `writer::day_dir(folder, date) -> PathBuf`; `writer::append_block(folder, block, dedup, shape, rules: &Rules) -> io::Result<()>`; `writer::render_website_row(block) -> String`; `writer::escape_cell(&str) -> String`.

- [ ] **Step 1: Write the failing tests** (replace every test that references `file_path`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Rules;
    use chrono::{Local, TimeZone};
    use tempfile::tempdir;

    fn date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 8, 25).unwrap()
    }

    fn block(app: &str, title: &str, url: Option<&str>, document: Option<&str>, minute: u32, end_minute: u32, lines: &[&str]) -> Block {
        Block {
            app: app.to_string(),
            title: Some(title.to_string()),
            document: document.map(str::to_string),
            url: url.map(str::to_string),
            start: Local.with_ymd_and_hms(2026, 8, 25, 9, minute, 0).unwrap(),
            end: Local.with_ymd_and_hms(2026, 8, 25, 9, end_minute, 0).unwrap(),
            lines: lines.iter().map(|s| s.to_string()).collect(),
            headings_only: false,
        }
    }

    fn zed() -> Block {
        block("Zed", "writer.rs", None, Some("/Users/x/writer.rs"), 14, 41, &["fn append_block"])
    }
    fn arc() -> Block {
        block("Arc", "Tauri | system tray", Some("https://v2.tauri.app/learn/system-tray/"), None, 41, 48, &["Tray icons on macOS"])
    }
    fn slack() -> Block {
        block("Slack", "#empty-build", None, None, 48, 59, &["dan: shipping the notch state thursday"])
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

    #[test]
    fn day_dir_is_days_slash_date() {
        assert_eq!(day_dir(Path::new("/tmp/x"), date()), PathBuf::from("/tmp/x/Days/2026-08-25"));
        assert_eq!(DayFile::Apps.path(Path::new("/tmp/x"), date()), PathBuf::from("/tmp/x/Days/2026-08-25/apps.md"));
    }

    #[test]
    fn three_blocks_land_in_three_files() {
        let dir = tempdir().unwrap();
        write_all(dir.path());

        let apps = read(dir.path(), DayFile::Apps);
        assert!(apps.starts_with("---\ndate: 2026-08-25\nkind: apps\ncaptured_by: Ambient Context "));
        assert!(apps.contains("## 09:14\u{2013}09:41 \u{00b7} Zed \u{00b7} writer.rs\n\nfile: /Users/x/writer.rs\n\nfn append_block\n"));
        assert!(apps.contains("## 09:41\u{2013}09:48 \u{00b7} Arc \u{00b7} Tauri | system tray\n\nrouted: websites\n"));
        assert!(apps.contains("## 09:48\u{2013}09:59 \u{00b7} Slack \u{00b7} #empty-build\n\nrouted: messages\n"));
        assert!(!apps.contains("Tray icons on macOS"));
        assert!(!apps.contains("dan: shipping"));

        let websites = read(dir.path(), DayFile::Websites);
        assert!(websites.contains("kind: websites\n"));
        assert!(websites.contains("| start | end | app | domain | title | url |\n| --- | --- | --- | --- | --- | --- |\n"));
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
        append_block(dir.path(), &page, &mut dedup, Shape::default(), &Rules::default()).unwrap();
        let mut editor = zed();
        editor.lines = vec!["shared sentence here".to_string()];
        append_block(dir.path(), &editor, &mut dedup, Shape::default(), &Rules::default()).unwrap();
        assert!(read(dir.path(), DayFile::Apps).contains("shared sentence here"));
    }

    #[test]
    fn a_restart_reseeds_from_apps_and_messages_together() {
        let dir = tempdir().unwrap();
        write_all(dir.path());
        let mut fresh = DayDedup::new();
        let mut again = zed();
        again.lines = vec!["fn append_block".into(), "dan: shipping the notch state thursday".into(), "new line".into()];
        append_block(dir.path(), &again, &mut fresh, Shape::default(), &Rules::default()).unwrap();
        let apps = read(dir.path(), DayFile::Apps);
        assert_eq!(apps.matches("fn append_block").count(), 1);
        assert!(!apps.contains("dan: shipping"), "seen in messages.md already");
        assert!(apps.contains("new line"));
    }

    #[test]
    fn a_headings_only_message_block_writes_headings_to_both_files() {
        let dir = tempdir().unwrap();
        let mut quiet = slack();
        quiet.headings_only = true;
        append_block(dir.path(), &quiet, &mut DayDedup::new(), Shape::default(), &Rules::default()).unwrap();
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
        append_block(dir.path(), &page, &mut DayDedup::new(), Shape::default(), &Rules::default()).unwrap();
        assert!(read(dir.path(), DayFile::Websites).contains("| 09:41 | 09:48 | Arc |  | Tauri \\| system tray |  |\n"));
    }

    #[test]
    fn a_deleted_day_folder_means_a_fresh_start() {
        let dir = tempdir().unwrap();
        let mut dedup = DayDedup::new();
        append_block(dir.path(), &zed(), &mut dedup, Shape::default(), &Rules::default()).unwrap();
        fs::remove_dir_all(day_dir(dir.path(), date())).unwrap();
        append_block(dir.path(), &zed(), &mut dedup, Shape::default(), &Rules::default()).unwrap();
        assert!(read(dir.path(), DayFile::Apps).contains("fn append_block"));
    }

    #[test]
    fn renders_a_heading_with_time_range_app_and_title() {
        let out = render_block(&zed(), &zed().lines, Shape::default());
        assert!(out.contains("## 09:14\u{2013}09:41 \u{00b7} Zed \u{00b7} writer.rs"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test writer::`
Expected: FAIL (`DayFile`, `day_dir` and the five-argument `append_block` are missing).

- [ ] **Step 3: Implement the layout and rendering**

Replace `file_path` and `frontmatter` with:

```rust
use crate::route::{self, Kind};
use crate::rules::Rules;

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
    days_dir(folder).join(format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day()))
}

fn frontmatter(date: NaiveDate, file: DayFile) -> String {
    let mut out = format!(
        "---\ndate: {:04}-{:02}-{:02}\nkind: {}\ncaptured_by: Ambient Context {}\n---\n",
        date.year(), date.month(), date.day(), file.kind_name(), env!("CARGO_PKG_VERSION")
    );
    if file == DayFile::Websites {
        out.push_str("\n| start | end | app | domain | title | url |\n| --- | --- | --- | --- | --- | --- |\n");
    }
    out
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

/// The heading alone, then where the body went. References are left off:
/// the website row or the messages block carries them.
fn render_routed(block: &Block, kind: Kind) -> String {
    let mut out = render_block(block, &[], Shape { max_block_chars: 0, write_references: false });
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
```

`render_block` is unchanged. One caveat: for a headings-only block `render_block` returns straight after the references, so `render_routed` must append `routed:` after it regardless; the code above does.

- [ ] **Step 4: Rewrite `DayDedup::roll_to` and the fresh-start check**

```rust
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
                self.skeletons.insert(Self::hash(&crate::prune::skeleton(line)));
            }
        }
    }
}
```

In `novel_lines`, the fresh-start check becomes `if !self.seen.is_empty() && !day_dir(folder, date).exists()`.

- [ ] **Step 5: Rewrite `append_block`**

```rust
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
    fs::create_dir_all(day_dir(folder, date))?;
    let kind = route::kind(rules, &block.app, block.title.as_deref(), block.url.as_deref());
    let apps = DayFile::Apps.path(folder, date);

    match kind {
        Kind::App => {
            let novel = if block.headings_only { Vec::new() } else { dedup.novel_lines(folder, block) };
            append_to(&apps, date, DayFile::Apps, &render_block(block, &novel, shape))?;
        }
        Kind::Website => {
            append_to(&apps, date, DayFile::Apps, &render_routed(block, kind))?;
            append_to(&DayFile::Websites.path(folder, date), date, DayFile::Websites, &render_website_row(block))?;
        }
        Kind::Message => {
            append_to(&apps, date, DayFile::Apps, &render_routed(block, kind))?;
            let novel = if block.headings_only { Vec::new() } else { dedup.novel_lines(folder, block) };
            append_to(&DayFile::Messages.path(folder, date), date, DayFile::Messages, &render_block(block, &novel, shape))?;
        }
    }
    ensure_agents_file(folder)
}
```

- [ ] **Step 6: Update `capture.rs` call sites and `is_own_output`**

All four `writer::append_block(&folder, &block, &mut dedup, shape)` calls become `writer::append_block(&folder, &block, &mut dedup, shape, &rules)`.

Replace `is_own_output` (and drop its `today` argument at the call site):

```rust
/// The app must not record itself reading its own record. Matched by the
/// document or URL path first (editors expose AXDocument), then by a
/// window title carrying a date together with the folder name or one of
/// the record's file names.
fn is_own_output(snapshot: &Snapshot, folder: &Path) -> bool {
    let folder_str = folder.to_string_lossy();
    if snapshot.document.as_deref().is_some_and(|d| d.contains(folder_str.as_ref())) {
        return true;
    }
    if snapshot.url.as_deref().is_some_and(|u| u.contains(folder_str.as_ref())) {
        return true;
    }
    let Some(title) = &snapshot.window_title else {
        return false;
    };
    static DATE: OnceLock<Regex> = OnceLock::new();
    let date = DATE.get_or_init(|| Regex::new(r"\d{4}-\d{2}-\d{2}").unwrap());
    if !date.is_match(title) {
        return false;
    }
    let folder_name = folder.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    const OWN_FILES: &[&str] = &["apps.md", "websites.md", "messages.md", "manifest.md"];
    (!folder_name.is_empty() && title.contains(folder_name.as_str()))
        || OWN_FILES.iter().any(|f| title.contains(f))
}
```

Add `use regex::Regex;` and `use std::sync::OnceLock;` at the top of `capture.rs`. Tests in `capture.rs`:

```rust
#[test]
fn a_kb_file_under_the_capture_folder_is_own_output() {
    let snap = Snapshot { app: "Zed".into(), document: Some("/Users/x/Ambient Context/KB/2026-09-02/threads.md".into()), ..Default::default() };
    assert!(is_own_output(&snap, Path::new("/Users/x/Ambient Context")));
}

#[test]
fn a_title_with_a_date_and_a_record_file_name_is_own_output() {
    let snap = Snapshot { app: "Obsidian".into(), window_title: Some("2026-09-02/messages.md".into()), ..Default::default() };
    assert!(is_own_output(&snap, Path::new("/Users/x/Ambient Context")));
    let other = Snapshot { app: "Obsidian".into(), window_title: Some("messages.md".into()), ..Default::default() };
    assert!(!is_own_output(&other, Path::new("/Users/x/Ambient Context")), "no date, not ours");
}
```

- [ ] **Step 7: Make the other `writer::file_path` callers compile**

`days.rs`, `jobs.rs`, `lib.rs` (`target_path`, `reveal_day`) and `mcp/files.rs` reference `writer::file_path`. Tasks 4, 6 and 10 rewrite them properly; here, replace each `writer::file_path(folder, date)` with `writer::DayFile::Apps.path(folder, date)`. The `jobs.rs` test helper becomes:

```rust
fn write_day(folder: &std::path::Path, date: NaiveDate) {
    let path = crate::writer::DayFile::Apps.path(folder, date);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, "---\ndate: 2026-08-28\nkind: apps\n---\n\n## 09:00\u{2013}11:00 \u{00b7} Linear\n\nread the issue\n").unwrap();
}
```

- [ ] **Step 8: Run the full Rust suite**

Run: `cd src-tauri && cargo test`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/writer.rs src-tauri/src/capture.rs src-tauri/src/days.rs src-tauri/src/jobs.rs src-tauri/src/lib.rs src-tauri/src/mcp/files.rs
git commit -m "Write each day as apps, websites and messages files under Days/"
```

---

### Task 3: Per-kind prune with the measured Message filters

**Files:**
- Modify: `src-tauri/src/prune.rs`
- Modify: `src-tauri/src/writer.rs` (`append_block`, the `Kind::Message` arm)

**Interfaces:**
- Produces: `prune::for_kind(kind: route::Kind, lines: Vec<String>) -> Vec<String>`, `prune::MAX_MESSAGE_LINE_CHARS: usize = 600`.

The filters come from measuring the eight days in `~/untitled folder/2026-08-*.md`: 123 lines that were only the object-replacement glyph `U+FFFC`, 80 `To:` and `Reply-To:` rows whose only value was that glyph, newsletter preheaders padded with soft hyphens (`U+00AD`), mailbox labels (`Inbox - cameron@empty.io email`, `All Inboxes \u{2013} 23 messages, 5 unread`), bare timestamps (`7:09 am`, `Yesterday at 11:15 pm`, `SEP 1`), and newsletter bodies captured as single lines over 600 characters.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn message_filters_drop_mail_chrome() {
    use crate::route::Kind;
    let input: Vec<String> = [
        "\u{fffc}",
        "To: \u{fffc}\u{fffc}",
        "Reply-To: \u{fffc}",
        "\u{fffc}Inbox - cameron@empty.io email",
        "All Inboxes \u{2013} 23 messages, 5 unread",
        "Inbox - cameron@standardretail.co",
        "7:09 am",
        "Yesterday at 11:15 pm",
        "Today at 9:41 am",
        "SEP 1",
        "Hi Lucy and Cameron I had a really positive phone call today",
    ].iter().map(|s| s.to_string()).collect();
    let out = for_kind(Kind::Message, input);
    assert_eq!(out, vec!["Hi Lucy and Cameron I had a really positive phone call today".to_string()]);
}

#[test]
fn message_filters_strip_soft_hyphen_padding_and_cut_long_lines() {
    use crate::route::Kind;
    let padded = format!("Get access to Delta today {} You signed up for early access", "\u{ad} ".repeat(60));
    let long = "word ".repeat(200);
    let out = for_kind(Kind::Message, vec![padded, long]);
    assert_eq!(out[0], "Get access to Delta today You signed up for early access");
    assert!(out[1].chars().count() <= MAX_MESSAGE_LINE_CHARS + " [cut]".len());
    assert!(out[1].ends_with(" [cut]"));
}

#[test]
fn app_and_website_kinds_are_left_alone() {
    use crate::route::Kind;
    let lines = vec!["7:09 am".to_string(), "\u{fffc}".to_string()];
    assert_eq!(for_kind(Kind::App, lines.clone()), lines);
    assert_eq!(for_kind(Kind::Website, lines.clone()), lines);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test prune::`
Expected: FAIL (`for_kind` missing).

- [ ] **Step 3: Implement**

```rust
use crate::route::Kind;

/// Newsletter bodies arrive as one enormous line. Past this many
/// characters the rest is cut: the first sentences carry the subject and
/// the sender, and the rest is the newsletter.
pub const MAX_MESSAGE_LINE_CHARS: usize = 600;

fn is_message_chrome(line: &str) -> bool {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        vec![
            // Mail header rows whose value was an attachment glyph.
            Regex::new(r"^(To|Cc|Bcc|From|Reply-To):\s*$").unwrap(),
            // Mailbox labels and the mailbox window title echoed as text.
            Regex::new(r"^(All Inboxes|Inbox|Sent|Drafts|Archive|Junk|Trash|Flagged)(\s*[-\x{2013}]\s*.*)?$").unwrap(),
            // Bare timestamps as Mail lists them.
            Regex::new(r"(?i)^\d{1,2}:\d{2} (am|pm)$").unwrap(),
            Regex::new(r"(?i)^(yesterday|today) at \d{1,2}:\d{2} (am|pm)$").unwrap(),
            Regex::new(r"^[A-Z]{3} \d{1,2}$").unwrap(),
        ]
    });
    patterns.iter().any(|p| p.is_match(line))
}

fn clean_message_line(line: &str) -> Option<String> {
    let stripped: String = line
        .chars()
        .filter(|c| *c != '\u{fffc}' && *c != '\u{ad}')
        .collect();
    let collapsed = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() || is_message_chrome(&collapsed) {
        return None;
    }
    if collapsed.chars().count() > MAX_MESSAGE_LINE_CHARS {
        let head: String = collapsed.chars().take(MAX_MESSAGE_LINE_CHARS).collect();
        return Some(format!("{} [cut]", head.trim_end()));
    }
    Some(collapsed)
}

/// A second pass at block close, once the block's kind is known. App and
/// Website lines pass through; Message lines lose the mail chrome the
/// snapshot-time filter cannot see without knowing the kind.
pub fn for_kind(kind: Kind, lines: Vec<String>) -> Vec<String> {
    match kind {
        Kind::Message => lines.iter().filter_map(|l| clean_message_line(l)).collect(),
        Kind::App | Kind::Website => lines,
    }
}
```

- [ ] **Step 4: Call it from the writer**

The `Kind::Message` arm of `append_block` becomes:

```rust
Kind::Message => {
    append_to(&apps, date, DayFile::Apps, &render_routed(block, kind))?;
    let cleaned = Block { lines: crate::prune::for_kind(kind, block.lines.clone()), ..block.clone() };
    let novel = if cleaned.headings_only { Vec::new() } else { dedup.novel_lines(folder, &cleaned) };
    append_to(&DayFile::Messages.path(folder, date), date, DayFile::Messages, &render_block(&cleaned, &novel, shape))?;
}
```

Add to the writer tests:

```rust
#[test]
fn message_bodies_are_pruned_of_mail_chrome_before_writing() {
    let dir = tempdir().unwrap();
    let mail = block("Mail", "All Inboxes", None, None, 10, 12, &["7:09 am", "Reply-To: \u{fffc}", "Patient letter regarding Mr Smith"]);
    append_block(dir.path(), &mail, &mut DayDedup::new(), Shape::default(), &Rules::default()).unwrap();
    let messages = read(dir.path(), DayFile::Messages);
    assert!(messages.contains("Patient letter regarding Mr Smith"));
    assert!(!messages.contains("7:09 am"));
    assert!(!messages.contains("Reply-To"));
}
```

- [ ] **Step 5: Run tests**

Run: `cd src-tauri && cargo test prune:: writer::`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/prune.rs src-tauri/src/writer.rs
git commit -m "Prune mail chrome from message blocks at block close"
```

---
### Task 4: Reading the folder layout in `days.rs`

**Files:**
- Modify: `src-tauri/src/days.rs` (whole file)
- Modify: `src-tauri/src/summarise.rs` (`list_captured` at line 169)

**Interfaces:**
- Consumes: `writer::DayFile`, `writer::days_dir`, `writer::day_dir`.
- Produces: `days::DayEntry { date, has_capture, has_summary, has_kb, bytes, title }`; `days::read_day(folder, date, file: DayFile) -> Option<String>`; `days::timeline(folder, date) -> Option<String>`; `days::spans(timeline: &str) -> Vec<(u32, u32)>` (minutes since midnight, end may exceed 1440 across midnight); `days::UrlTotal { url, domain, title, dwell_secs: u64, visits: u32, first, last }`; `days::website_totals(folder, date) -> Vec<UrlTotal>`; `days::render_totals(&[UrlTotal]) -> String`; `days::RawBlock` gains `routed: Option<String>`; `days::parse_blocks` unchanged in name. `has_kb` is `false` until Task 9 provides `ingest::has_kb`; wire it here as `crate::ingest::has_kb` only if Task 9 has landed, otherwise `false` with a `// Task 10` comment.

- [ ] **Step 1: Write the failing tests** (replace the existing `folder()` fixture)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::DayFile;
    use tempfile::tempdir;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn write(dir: &Path, date: NaiveDate, file: DayFile, text: &str) {
        let path = file.path(dir, date);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    const APPS: &str = "---\ndate: 2026-08-27\nkind: apps\n---\n\n## 09:00\u{2013}09:30 \u{00b7} Zed \u{00b7} writer.rs\n\nfile: /x/writer.rs\n\nfn a\n\n## 09:30\u{2013}09:41 \u{00b7} Arc \u{00b7} Tauri\n\nrouted: websites\n\n## 23:50\u{2013}00:10 \u{00b7} Slack \u{00b7} #x\n\nrouted: messages\n";

    const WEBSITES: &str = "---\ndate: 2026-08-27\nkind: websites\n---\n\n| start | end | app | domain | title | url |\n| --- | --- | --- | --- | --- | --- |\n| 09:30 | 09:41 | Arc | v2.tauri.app | Tauri | https://v2.tauri.app/ |\n| 10:00 | 10:05 | Arc | v2.tauri.app | Tauri again | https://v2.tauri.app/ |\n| 10:05 | 10:06 | Arc |  | Loading \\| page |  |\n| 10:06 | 10:07 | Arc |  | Loading \\| page |  |\n";

    fn folder() -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        write(dir.path(), date(2026, 8, 27), DayFile::Apps, APPS);
        write(dir.path(), date(2026, 8, 27), DayFile::Websites, WEBSITES);
        write(dir.path(), date(2026, 8, 28), DayFile::Apps, "---\n---\n");
        // A 0.1 flat file, which every reader ignores.
        std::fs::write(dir.path().join("2026-08-20.md"), "old").unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "x").unwrap();
        std::fs::create_dir_all(dir.path().join("Summaries")).unwrap();
        std::fs::write(
            dir.path().join("Summaries").join("2026-08-27.md"),
            "---\ndate: 2026-08-27\n---\n\n# A day of plumbing\n\nprose",
        )
        .unwrap();
        dir
    }

    #[test]
    fn list_days_reads_folders_and_ignores_flat_files() {
        let dir = folder();
        let days = list_days(dir.path());
        let dates: Vec<String> = days.iter().map(|d| d.date.to_string()).collect();
        assert_eq!(dates, vec!["2026-08-28", "2026-08-27"]);
        let first = &days[1];
        assert!(first.has_capture);
        assert!(first.has_summary);
        assert_eq!(first.bytes, (APPS.len() + WEBSITES.len()) as u64);
        assert_eq!(first.title.as_deref(), Some("A day of plumbing"));
    }

    #[test]
    fn read_day_returns_one_file() {
        let dir = folder();
        assert_eq!(read_day(dir.path(), date(2026, 8, 27), DayFile::Websites).unwrap(), WEBSITES);
        assert!(read_day(dir.path(), date(2026, 8, 27), DayFile::Messages).is_none());
    }

    #[test]
    fn timeline_is_headings_only() {
        let dir = folder();
        let out = timeline(dir.path(), date(2026, 8, 27)).unwrap();
        assert_eq!(out.lines().count(), 3);
        assert!(out.lines().all(|l| l.starts_with("## ")));
        assert!(!out.contains("routed:"));
    }

    #[test]
    fn spans_parse_minutes_and_cross_midnight() {
        let out = spans("## 09:00\u{2013}09:30 \u{00b7} Zed\n## 23:50\u{2013}00:10 \u{00b7} Slack\n");
        assert_eq!(out, vec![(540, 570), (1430, 1450)]);
    }

    #[test]
    fn website_totals_merge_by_url_and_rank_by_dwell() {
        let dir = folder();
        let totals = website_totals(dir.path(), date(2026, 8, 27));
        assert_eq!(totals.len(), 2);
        assert_eq!(totals[0].url, "https://v2.tauri.app/");
        assert_eq!(totals[0].dwell_secs, 16 * 60);
        assert_eq!(totals[0].visits, 2);
        assert_eq!(totals[0].title, "Tauri", "title of the longest visit");
        assert_eq!((totals[0].first.as_str(), totals[0].last.as_str()), ("09:30", "10:05"));
        assert_eq!(totals[1].title, "Loading | page", "empty-url rows merge by title, unescaped");
        assert_eq!(totals[1].visits, 2);
    }

    #[test]
    fn render_totals_is_a_pipe_table_with_dwell_in_minutes() {
        let dir = folder();
        let out = render_totals(&website_totals(dir.path(), date(2026, 8, 27)));
        assert!(out.starts_with("| domain | title | dwell | visits | first | last | url |\n| --- | --- | --- | --- | --- | --- | --- |\n"));
        assert!(out.contains("| v2.tauri.app | Tauri | 16m | 2 | 09:30 | 10:05 | https://v2.tauri.app/ |\n"));
        assert!(out.contains("| Loading \\| page |"));
    }

    #[test]
    fn parse_blocks_keeps_routed_out_of_the_body() {
        let blocks = parse_blocks(APPS);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[1].routed.as_deref(), Some("websites"));
        assert!(blocks[1].lines.is_empty());
        assert_eq!(blocks[0].file.as_deref(), Some("/x/writer.rs"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test days::`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
use crate::writer::{self, DayFile};
use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DayEntry {
    pub date: NaiveDate,
    pub has_capture: bool,
    pub has_summary: bool,
    pub has_kb: bool,
    pub bytes: u64,
    pub title: Option<String>,
}

fn entry(folder: &Path, date: NaiveDate) -> DayEntry {
    let summary = std::fs::read_to_string(crate::summarise::summary_path(folder, date)).ok();
    let bytes = DayFile::all()
        .iter()
        .filter_map(|file| std::fs::metadata(file.path(folder, date)).ok())
        .map(|m| m.len())
        .sum();
    DayEntry {
        date,
        has_capture: DayFile::Apps.path(folder, date).is_file(),
        has_summary: summary.is_some(),
        has_kb: false, // Task 9: crate::ingest::has_kb(folder, date)
        bytes,
        title: summary.as_deref().and_then(crate::summarise::title_of),
    }
}

fn known_dates(folder: &Path) -> BTreeSet<NaiveDate> {
    let mut dates: BTreeSet<NaiveDate> = crate::summarise::list_captured(folder).into_iter().collect();
    dates.extend(crate::summarise::list_summarised(folder));
    dates
}

pub fn list_days(folder: &Path) -> Vec<DayEntry> {
    known_dates(folder).into_iter().rev().map(|date| entry(folder, date)).collect()
}

pub fn days_in_month(folder: &Path, year: i32, month: u32) -> Vec<DayEntry> {
    known_dates(folder)
        .into_iter()
        .filter(|date| date.year() == year && date.month() == month)
        .map(|date| entry(folder, date))
        .collect()
}

pub fn read_day(folder: &Path, date: NaiveDate, file: DayFile) -> Option<String> {
    std::fs::read_to_string(file.path(folder, date)).ok()
}

pub fn read_summary(folder: &Path, date: NaiveDate) -> Option<String> {
    std::fs::read_to_string(crate::summarise::summary_path(folder, date)).ok()
}

/// The `## ` headings of apps.md, one per line: the day's clock.
pub fn timeline(folder: &Path, date: NaiveDate) -> Option<String> {
    let text = read_day(folder, date, DayFile::Apps)?;
    let mut out = String::new();
    for line in text.lines().filter(|l| l.starts_with("## ")) {
        out.push_str(line);
        out.push('\n');
    }
    Some(out)
}

fn minutes(hhmm: &str) -> Option<u32> {
    let (h, m) = hhmm.split_once(':')?;
    Some(h.parse::<u32>().ok()? * 60 + m.parse::<u32>().ok()?)
}

/// `(start, end)` in minutes since midnight for every heading. An end
/// before its start crossed midnight and is carried past 1440.
pub fn spans(timeline: &str) -> Vec<(u32, u32)> {
    timeline
        .lines()
        .filter_map(|line| {
            let (start, end, _, _) = parse_heading(line)?;
            let s = minutes(&start)?;
            let mut e = minutes(&end)?;
            if e < s {
                e += 24 * 60;
            }
            Some((s, e))
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UrlTotal {
    pub url: String,
    pub domain: String,
    pub title: String,
    pub dwell_secs: u64,
    pub visits: u32,
    pub first: String,
    pub last: String,
}

fn unescape_cell(cell: &str) -> String {
    cell.replace("\\|", "|")
}

/// Splits a table row on unescaped pipes. The leading and trailing
/// separators produce empty first and last cells, which are dropped.
fn row_cells(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'|') {
            current.push_str("\\|");
            chars.next();
        } else if c == '|' {
            cells.push(unescape_cell(current.trim()));
            current.clear();
        } else {
            current.push(c);
        }
    }
    cells.push(unescape_cell(current.trim()));
    if cells.len() >= 2 {
        cells.remove(0);
        cells.pop();
    }
    cells
}

pub fn website_totals(folder: &Path, date: NaiveDate) -> Vec<UrlTotal> {
    let Some(text) = read_day(folder, date, DayFile::Websites) else {
        return Vec::new();
    };
    let mut totals: Vec<UrlTotal> = Vec::new();
    let mut longest: Vec<u64> = Vec::new();
    for line in text.lines().filter(|l| l.starts_with("| ") && !l.starts_with("| start") && !l.starts_with("| ---")) {
        let cells = row_cells(line);
        if cells.len() != 6 {
            continue;
        }
        let (Some(s), Some(e)) = (minutes(&cells[0]), minutes(&cells[1])) else {
            continue;
        };
        let e = if e < s { e + 24 * 60 } else { e };
        let dwell = u64::from(e - s) * 60;
        let url = cells[5].clone();
        let key_title = cells[4].clone();
        let position = totals.iter().position(|t| {
            if url.is_empty() { t.url.is_empty() && t.title == key_title } else { t.url == url }
        });
        match position {
            Some(i) => {
                totals[i].dwell_secs += dwell;
                totals[i].visits += 1;
                totals[i].last = cells[1].clone();
                if dwell > longest[i] {
                    longest[i] = dwell;
                    totals[i].title = key_title;
                }
            }
            None => {
                totals.push(UrlTotal {
                    url,
                    domain: cells[3].clone(),
                    title: key_title,
                    dwell_secs: dwell,
                    visits: 1,
                    first: cells[0].clone(),
                    last: cells[1].clone(),
                });
                longest.push(dwell);
            }
        }
    }
    totals.sort_by(|a, b| b.dwell_secs.cmp(&a.dwell_secs).then(a.first.cmp(&b.first)));
    totals
}

pub fn render_totals(totals: &[UrlTotal]) -> String {
    let mut out = String::from("| domain | title | dwell | visits | first | last | url |\n| --- | --- | --- | --- | --- | --- | --- |\n");
    for t in totals {
        out.push_str(&format!(
            "| {} | {} | {}m | {} | {} | {} | {} |\n",
            writer::escape_cell(&t.domain),
            writer::escape_cell(&t.title),
            t.dwell_secs / 60,
            t.visits,
            t.first,
            t.last,
            writer::escape_cell(&t.url),
        ));
    }
    out
}
```

`RawBlock` gains `pub routed: Option<String>` (initialise `None` in `parse_blocks`), and `parse_blocks` gets one more arm before the body push: `else if let Some(name) = line.strip_prefix("routed: ") { block.routed = Some(name.to_string()); }`. `parse_heading` is unchanged.

In `summarise.rs`, `list_captured` becomes:

```rust
/// Every date with a Days/ folder. Flat 0.1 day files are ignored.
pub fn list_captured(folder: &Path) -> Vec<NaiveDate> {
    let Ok(entries) = std::fs::read_dir(crate::writer::days_dir(folder)) else {
        return Vec::new();
    };
    let mut dates: Vec<NaiveDate> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| NaiveDate::parse_from_str(&entry.file_name().to_string_lossy(), "%Y-%m-%d").ok())
        .collect();
    dates.sort();
    dates
}
```

- [ ] **Step 4: Fix the callers**

`mcp/files.rs` `list_days` JSON gains `"has_kb": day.has_kb`. `mcp/files.rs` `read_day` becomes `read_day(folder, date, file: DayFile, from, to)` and calls `crate::days::read_day(folder, date, file)`; Task 6 finishes the tool surface. `lib.rs` `read_day` command: `days::read_day(&folder, date, writer::DayFile::Apps)` for now (Task 5 adds the argument). `lib.rs` `read_day_blocks` likewise.

- [ ] **Step 5: Run tests**

Run: `cd src-tauri && cargo test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/days.rs src-tauri/src/summarise.rs src-tauri/src/lib.rs src-tauri/src/mcp/files.rs
git commit -m "Read the Days/ layout, the timeline and website totals"
```

---
### Task 5: Day view reads three files

**Files:**
- Modify: `src-tauri/src/lib.rs` (`target_path` at line 186, `reveal_day` at line 260, `read_day` at line 1219, `read_day_blocks` at line 463, `generate_handler!`)
- Modify: `src/lib/days.ts`, `src/components/DayView.tsx`, `src/components/DayHeader.tsx`, `src/components/RawPane.tsx`
- Create: `src/components/WebsitesPane.tsx`
- Modify: `src/test/DayView.test.tsx`, `src/test/RawPane.test.tsx`; create `src/test/WebsitesPane.test.tsx`

**Interfaces:**
- Consumes: `days::read_day(folder, date, DayFile)`, `days::website_totals`, `days::UrlTotal`, `writer::DayFile::from_name`.
- Produces: Tauri commands `read_day(date, file?: string)`, `read_day_blocks(date, file?: string)`, `website_totals(date) -> Vec<UrlTotal>`; `open_in_editor(date, which)` accepts `apps`, `websites`, `messages`, `summary`; `reveal_day` opens `Days/{date}/`. Frontend: `DayFile = "apps" | "websites" | "messages"`, `UrlTotal`, `RawPane` prop `file: DayFile`, `WebsitesPane({ date })`, `DayHeader` props `rawFile`, `onRawFile`.

- [ ] **Step 1: Rust commands**

```rust
fn parse_day_file(file: Option<String>) -> Result<writer::DayFile, String> {
    match file.as_deref() {
        None => Ok(writer::DayFile::Apps),
        Some(name) => writer::DayFile::from_name(name).ok_or_else(|| format!("{name} is not one of apps, websites or messages")),
    }
}

#[tauri::command]
fn read_day(app: tauri::AppHandle, date: String, file: Option<String>) -> Option<String> {
    let folder = settings::load(&app).folder?;
    let file = parse_day_file(file).ok()?;
    days::read_day(&folder, parse_date(&date).ok()?, file)
}

#[tauri::command]
fn read_day_blocks(app: tauri::AppHandle, date: String, file: Option<String>) -> Vec<days::RawBlock> {
    let Some(folder) = settings::load(&app).folder else { return Vec::new(); };
    let (Ok(date), Ok(file)) = (parse_date(&date), parse_day_file(file)) else { return Vec::new(); };
    days::read_day(&folder, date, file).map(|text| days::parse_blocks(&text)).unwrap_or_default()
}

#[tauri::command]
fn website_totals(app: tauri::AppHandle, date: String) -> Vec<days::UrlTotal> {
    let Some(folder) = settings::load(&app).folder else { return Vec::new(); };
    let Ok(date) = parse_date(&date) else { return Vec::new(); };
    days::website_totals(&folder, date)
}
```

`target_path`:

```rust
match which {
    "apps" | "websites" | "messages" => writer::DayFile::from_name(which).map(|f| f.path(folder, date)),
    "summary" => Some(summarise::summary_path(folder, date)),
    _ => None,
}
```

`reveal_day`: `let target = writer::day_dir(&folder, parsed); let target = if target.is_dir() { target } else { folder };` and open with `open` (no `-R`, it is a folder). Register `website_totals` in `generate_handler!`.

Tests in `lib.rs` (module `tests` already exists for `target_path`; extend or add):

```rust
#[test]
fn target_path_names_the_three_day_files_and_the_summary() {
    let f = std::path::Path::new("/f");
    let d = chrono::NaiveDate::from_ymd_opt(2026, 9, 2).unwrap();
    assert_eq!(target_path(f, d, "messages").unwrap(), std::path::PathBuf::from("/f/Days/2026-09-02/messages.md"));
    assert_eq!(target_path(f, d, "summary").unwrap(), std::path::PathBuf::from("/f/Summaries/2026-09-02.md"));
    assert!(target_path(f, d, "day").is_none());
}
```

- [ ] **Step 2: Frontend types in `src/lib/days.ts`**

```ts
export type DayFile = "apps" | "websites" | "messages";

export type UrlTotal = {
  url: string;
  domain: string;
  title: string;
  dwell_secs: number;
  visits: number;
  first: string;
  last: string;
};
```

Add `has_kb: boolean;` to `DayEntry`.

- [ ] **Step 3: Failing frontend tests**

`src/test/WebsitesPane.test.tsx`:

```tsx
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { mockInvoke } from "./tauri-mock";
import { WebsitesPane } from "../components/WebsitesPane";

vi.mock("@tauri-apps/api/core", async () => {
  const mock = await import("./tauri-mock");
  return { invoke: mock.invoke };
});

describe("WebsitesPane", () => {
  afterEach(cleanup);

  it("renders totals ranked by dwell with minutes and visits", async () => {
    mockInvoke((command) => {
      if (command === "website_totals") {
        return [
          { url: "https://v2.tauri.app/", domain: "v2.tauri.app", title: "Tauri", dwell_secs: 960, visits: 2, first: "09:30", last: "10:05" },
        ];
      }
      throw new Error(`unexpected command ${command}`);
    });
    render(<WebsitesPane date="2026-08-27" />);
    expect(await screen.findByText("v2.tauri.app")).toBeTruthy();
    expect(screen.getByText("16m")).toBeTruthy();
    expect(screen.getByText("2")).toBeTruthy();
    expect(screen.getByTitle("https://v2.tauri.app/")).toBeTruthy();
  });

  it("says so when nothing was visited", async () => {
    mockInvoke((command) => {
      if (command === "website_totals") return [];
      throw new Error(`unexpected command ${command}`);
    });
    render(<WebsitesPane date="2026-08-27" />);
    expect(await screen.findByText("No websites recorded.")).toBeTruthy();
  });
});
```

In `src/test/DayView.test.tsx`, the `handler` gains `case "website_totals": return [];` and the `read_day` case stays. Add:

```tsx
it("switches the raw pane between apps, websites and messages", async () => {
  mockInvoke(handler(null));
  render(<DayView />);
  await waitFor(() => expect(countOf("read_day_blocks")).toBeGreaterThan(0));
  fireEvent.click(screen.getByRole("tab", { name: "Websites" }));
  await waitFor(() => expect(countOf("website_totals")).toBeGreaterThan(0));
  fireEvent.click(screen.getByRole("tab", { name: "Messages" }));
  await waitFor(() =>
    expect(callsOf("read_day_blocks").some((call) => call.args?.file === "messages")).toBe(true),
  );
});
```

Import `fireEvent` from `@testing-library/react`. In `RawPane.test.tsx`, every `render(<RawPane date=... mode="raw" />)` gains `file="apps"`, and one new assertion: `expect(callsOf("read_day_blocks")[0].args?.file).toBe("apps")` (import `callsOf`).

- [ ] **Step 4: Run to verify they fail**

Run: `npx vitest run src/test/WebsitesPane.test.tsx src/test/DayView.test.tsx src/test/RawPane.test.tsx`
Expected: FAIL.

- [ ] **Step 5: `WebsitesPane.tsx`**

```tsx
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { UrlTotal } from "../lib/days";

function minutes(secs: number): string {
  return `${Math.round(secs / 60)}m`;
}

export function WebsitesPane({ date }: { date: string }) {
  const [totals, setTotals] = useState<UrlTotal[] | null>(null);

  useEffect(() => {
    let cancelled = false;
    void invoke<UrlTotal[]>("website_totals", { date }).then((next) => {
      if (!cancelled) setTotals(next);
    });
    return () => {
      cancelled = true;
    };
  }, [date]);

  if (totals === null) return null;
  if (totals.length === 0) {
    return (
      <section className="websites-pane">
        <p className="pane-empty">No websites recorded.</p>
      </section>
    );
  }
  return (
    <section className="websites-pane">
      <div className="websites-pane-scroll">
        <table className="websites-table">
          <thead>
            <tr>
              <th>Domain</th>
              <th>Title</th>
              <th>Dwell</th>
              <th>Visits</th>
              <th>First</th>
              <th>Last</th>
            </tr>
          </thead>
          <tbody>
            {totals.map((row, index) => (
              <tr key={`${row.url}-${row.title}-${index}`} title={row.url}>
                <td>{row.domain || "(no url)"}</td>
                <td>
                  {row.url ? (
                    <a href={row.url} onClick={(event) => { event.preventDefault(); void invoke("open_link", { url: row.url }); }}>
                      {row.title}
                    </a>
                  ) : (
                    row.title
                  )}
                </td>
                <td className="num">{minutes(row.dwell_secs)}</td>
                <td className="num">{row.visits}</td>
                <td className="num">{row.first}</td>
                <td className="num">{row.last}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
```

Add to `src/main-window.css` beside `.raw-pane`: `.websites-pane { display: flex; flex-direction: column; min-height: 0; }`, `.websites-pane-scroll { overflow: auto; }`, `.websites-table { width: 100%; border-collapse: collapse; font-variant-numeric: tabular-nums; }`, `.websites-table th, .websites-table td { text-align: left; padding: 4px 8px; }`, `.websites-table .num { text-align: right; }`, `.pane-empty { padding: 16px; opacity: 0.7; }`.

- [ ] **Step 6: `RawPane` takes `file`; `DayView` and `DayHeader` gain the raw tabs**

`RawPane`: props become `{ date: string; mode: "raw" | "kb" | "summary"; file: DayFile }`; `read` calls `invoke<RawBlock[]>("read_day_blocks", { date, file })` and the effect depends on `[date, file, read, readRules]`.

`DayView`: `mode` state type becomes `"raw" | "kb" | "summary"` (`"kb"` renders nothing until Task 11; add `{mode === "kb" ? null : ...}` for now). Add `const [rawFile, setRawFile] = useState<DayFile>("apps");`. Every `invoke("read_day", { date: selected })` becomes `invoke("read_day", { date: selected, file: "apps" })` (the stats come from `apps.md`). Render:

```tsx
{mode === "summary" ? (
  <SummaryPane ... />
) : mode === "kb" ? null : rawFile === "websites" ? (
  <WebsitesPane date={selected} />
) : (
  <RawPane date={selected} mode={mode} file={rawFile} />
)}
```

Pass `rawFile={rawFile}` and `onRawFile={setRawFile}` to `DayHeader`.

`DayHeader`: props gain `rawFile: DayFile; onRawFile: (file: DayFile) => void;` and `mode`/`onMode` widen to the three-mode type. Under the existing segmented control, when `mode === "raw"`, render a second `div.segmented[role=tablist]` with three `button[role=tab]` labelled `Apps`, `Websites`, `Messages`, `aria-selected={rawFile === key}`. `onOpen` maps: raw mode opens `rawFile`; summary opens `"summary"`; kb opens `"kb"` (Task 11 makes the backend accept it).

- [ ] **Step 7: Run the frontend gates**

Run: `npx tsc --noEmit && npx vitest run`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/lib.rs src/lib/days.ts src/components/DayView.tsx src/components/DayHeader.tsx src/components/RawPane.tsx src/components/WebsitesPane.tsx src/main-window.css src/test/DayView.test.tsx src/test/RawPane.test.tsx src/test/WebsitesPane.test.tsx
git commit -m "Show apps, websites and messages as tabs in the Raw view"
```

---

### Task 6: MCP `read_day` file argument and search over three files

**Files:**
- Modify: `src-tauri/src/mcp/files.rs` (`read_day` at line 72, `search_record` at line 136)
- Modify: `src-tauri/src/mcp/tools.rs` (`read_day` def at line 89, `read_call` at line 310)
- Modify: `docs/mcp.md` (`read_day`, `list_days`, `search_record` sections)

**Interfaces:**
- Produces: MCP `read_day { date, file?, from?, to? }` with `file` one of `apps` (default), `websites`, `messages`; `search_record` hits carry `layer` of `apps`, `websites`, `messages` or `summary`.

- [ ] **Step 1: Failing tests in `tools.rs`**

The existing tests build a folder with a flat day file through a helper (look for `2026-08-30.md` in the tests). Change the helper to write `Days/2026-08-30/apps.md` via `crate::writer::DayFile::Apps.path(...)` (create the parent). Add:

```rust
#[test]
fn read_day_returns_the_named_file() {
    let dir = tempfile::tempdir().unwrap();
    let date = chrono::NaiveDate::from_ymd_opt(2026, 8, 30).unwrap();
    for (file, text) in [(crate::writer::DayFile::Apps, "apps text"), (crate::writer::DayFile::Websites, "websites text")] {
        let path = file.path(dir.path(), date);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let mut server = server_with_folder(dir.path());
    let out = call(&mut server, "read_day", &json!({ "date": "2026-08-30", "file": "websites" }));
    assert_eq!(out["content"][0]["text"], "websites text");
    let out = call(&mut server, "read_day", &json!({ "date": "2026-08-30", "file": "photos" }));
    assert!(out["isError"].as_bool().unwrap_or(false));
    let out = call(&mut server, "read_day", &json!({ "date": "2026-08-30", "file": "messages" }));
    assert!(out["isError"].as_bool().unwrap_or(false), "no messages.md that day");
}
```

Use whatever the existing test helper for a folder-only server is called (it is the one the `read_day` tests at line 532 use); the name above is a stand-in for that helper.

- [ ] **Step 2: Implement**

`files.rs`:

```rust
pub fn read_day(folder: &Path, date: NaiveDate, file: crate::writer::DayFile, from: Option<&str>, to: Option<&str>) -> Result<String, FileError> {
    let text = crate::days::read_day(folder, date, file).ok_or(FileError::NoCapture(date))?;
    // Time filtering only makes sense on block files.
    if (from.is_none() && to.is_none()) || file == crate::writer::DayFile::Websites {
        return Ok(text);
    }
    ...unchanged loop...
}
```

`search_record`: replace `sources` with

```rust
let mut sources: Vec<(&'static str, std::path::PathBuf)> = crate::writer::DayFile::all()
    .iter()
    .map(|f| (f.kind_name(), f.path(folder, day.date)))
    .collect();
sources.push(("summary", folder.join("Summaries").join(format!("{date}.md"))));
```

`tools.rs` def: add `"file": { "type": "string", "enum": ["apps", "websites", "messages"], "description": "Which of the day's files to return. apps is the timeline with native app bodies, websites is the visit table, messages is message bodies. Defaults to apps." }`. Description: "Returns one of the day's raw files exactly as on disk...". In `read_call`, parse `file` with `crate::writer::DayFile::from_name`, returning `tool_error("file must be one of apps, websites or messages")` otherwise.

`docs/mcp.md`: `read_day` documents `file`; `list_days` mentions `has_kb` and that `bytes` is the sum of the three files; `search_record` lists the four layer names.

- [ ] **Step 3: Run tests**

Run: `cd src-tauri && cargo test mcp:: && cargo test --test docs_match_tools`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/mcp/files.rs src-tauri/src/mcp/tools.rs docs/mcp.md
git commit -m "Expose the three day files over MCP read_day and search"
```

---
## PR 2: Ingest into KB/ and summarise from it

### Task 7: Settings for the ingest agent and input cap

**Files:**
- Modify: `src-tauri/src/settings.rs` (struct at line 20, `Default` at line 61)
- Modify: `src/lib/days.ts` (`Settings` type)
- Modify: `src-tauri/src/mcp/files.rs` `get_config` settable keys and `docs/mcp.md` `set_config` (add `ingest_max_chars`; `ingest_agent` is not settable over MCP, like `agent`)

**Interfaces:**
- Produces: `Settings.ingest_agent: Option<Agent>` (default `None`), `Settings.ingest_max_chars: usize` (default `400_000`).

- [ ] **Step 1: Failing test**

```rust
#[test]
fn ingest_fields_default_when_missing_from_the_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("settings.json");
    std::fs::write(&path, r#"{"interval_secs": 5}"#).unwrap();
    let settings = read_from(&path);
    assert_eq!(settings.ingest_agent, None);
    assert_eq!(settings.ingest_max_chars, 400_000);
}
```

- [ ] **Step 2: Implement**

Add to the struct, after `agent`:

```rust
/// Runs the three ingest calls. None means the summary agent runs them.
pub ingest_agent: Option<Agent>,
/// Cap on the input of one ingest call, in characters. Over it, the
/// longest block bodies are trimmed first.
pub ingest_max_chars: usize,
```

Defaults: `ingest_agent: None, ingest_max_chars: 400_000`. In `load`, also `normalize_claude_agent` on `ingest_agent` when present. Frontend `Settings` type gains `ingest_agent: Agent | null; ingest_max_chars: number;`. Existing tests that build a `Settings` literal gain the two fields.

- [ ] **Step 3: Run tests and commit**

Run: `cd src-tauri && cargo test settings:: && cd .. && npx tsc --noEmit`
Expected: PASS.

```bash
git add src-tauri/src/settings.rs src/lib/days.ts src-tauri/src/mcp/files.rs docs/mcp.md
git commit -m "Add ingest agent and ingest input cap settings"
```

---

### Task 8: Four prompts behind one `PromptId`

**Files:**
- Modify: `src-tauri/src/prompt.rs` (whole file)
- Create: `src-tauri/prompts/ingest-messages.md`, `ingest-apps.md`, `ingest-websites.md`
- Modify: `src-tauri/prompts/day-context.md` (input section and the tail)
- Modify: `src-tauri/src/lib.rs` (`prompt_payload`, `get_prompt`, `set_prompt`, `reset_prompt`, `prompt_editor_target`, `open_prompt_in_editor`), `src-tauri/src/control.rs` (`writes::set_prompt`), `src-tauri/src/mcp/files.rs` (`get_prompt`)
- Modify: `src-tauri/src/summarise.rs` (`build_prompt`, remove `BUNDLED_PROMPT`)

**Interfaces:**
- Produces: `prompt::PromptId { DayContext, IngestMessages, IngestApps, IngestWebsites }` with `all() -> [PromptId; 4]`, `as_str(self) -> &'static str` (`day-context`, `ingest-messages`, `ingest-apps`, `ingest-websites`), `parse(&str) -> Option<PromptId>`, `bundled(self) -> &'static str`, `placeholders(self) -> &'static [&'static str]`, `markers(self) -> &'static [&'static str]`; `prompt::prompt_path(config_dir, id)`, `is_customised(config_dir, id)`, `current(config_dir, id)`, `validate(id, text)`, `set(config_dir, id, text)`, `reset(config_dir, id)`; `PromptError::MissingPlaceholder(String)`, `PromptError::MissingMarker(String)`; `summarise::build_prompt(template, date, timeline, kb) -> String`; ingest output markers `<<<file: NAME>>>` and `<<<reasoning>>>`; Tauri `get_prompt(id?)`, `set_prompt(id?, text)`, `reset_prompt(id?)` where a missing id means `day-context`; `PromptPayload` gains `id`.

- [ ] **Step 1: Write the three ingest prompts**

`src-tauri/prompts/ingest-messages.md`:

````markdown
You are reading one day of message surfaces captured from the user's screen
(mail, chat, inboxes, pull request pages) and turning them into two short,
cited files. Other LLMs read your output to learn who the user dealt with
and what was agreed. Precision beats completeness.

**How to read the input:**

- Each `## HH:MM–HH:MM · App · Window title` heading is one stretch of
  attention on a message surface. The timeline below lists every heading
  of the day, including ones whose bodies are in other files; use it to
  place times.
- Body lines are accessibility-tree scrape in visual order: subject lines,
  senders, previews and fragments of bodies, deduplicated across the day.
  Treat them like OCR.
- `url:` lines are exact references. Cite them where they exist.
- Newsletters and notifications are not people. A person is someone who
  wrote to the user, or whom the user wrote to.

**Produce exactly this output, with no code fence and nothing before the
first marker:**

```
<<<file: people.md>>>
## <Person's name as it appears>
<1 to 5 lines: where (app, channel or thread), what was discussed, what
was asked or agreed. Every line ends with the time range that supports it,
written HH:MM-HH:MM, and a url: reference where one exists.>

<<<file: commitments.md>>>
## I agreed to
- [ ] <what> · with <whom> · HH:MM-HH:MM · <reference or none>

## Owed to me
- [ ] <what> · from <whom> · HH:MM-HH:MM · <reference or none>

<<<reasoning>>>
<2-4 sentences on what you treated as a person or a commitment and what
you left out.>
```

**Rules:**

- A file with nothing to report is exactly the line `Nothing evident.` and
  nothing else. Both file markers must always appear.
- Every non-heading line carries a time range `HH:MM-HH:MM` taken from a
  heading in the input or the timeline. A claim you cannot place in time
  is a claim to leave out.
- A commitment is a concrete future action visible in the text: "I'll send
  the invoice Thursday", "can you review this by Friday". Not a
  newsletter's call to action, not a marketing offer.
- Summarise sensitive content (health, finance, family) at the category
  level without reproducing details.
- At most 200 lines per file.

The date is {{DATE}}.

Timeline of the whole day:

{{TIMELINE}}

---

{{INPUT}}
````

`src-tauri/prompts/ingest-apps.md`:

````markdown
You are reading one day of captured application windows (editors,
terminals, notes, design tools, native apps) and turning them into three
short, cited files about the user's work. Other LLMs read your output to
learn what was worked on, which tools appeared, and what went wrong.

**How to read the input:**

- Each `## HH:MM–HH:MM · App · Window title` heading is one stretch of
  attention. Headings alone are the timeline; read them all first.
- A `file:` line is the exact path of the document behind the block.
  Cite it in preference to quoting fragments.
- A block whose body is only `routed: websites` or `routed: messages` was
  a web page or a message surface. It still counts for time; its content
  is in another file you do not have.
- Body lines are deduplicated across the day and are accessibility-tree
  scrape: visual order, interface residue, partial viewport. A block with
  no body is a return to something already recorded, never "nothing".
- Time is the strongest signal. Repeated returns to the same file matter
  more than any single visit.

**Produce exactly this output, with no code fence and nothing before the
first marker:**

```
<<<file: threads.md>>>
## <Thread: a project, ticket or piece of work>
<1 to 5 lines: what was done, files touched, what changed or was decided.
Every line ends with HH:MM-HH:MM and a file: reference where one exists.>

<<<file: products.md>>>
## <Product, library or service>
<1 to 3 lines on how it appeared: used, evaluated, configured, mentioned.
Every line ends with HH:MM-HH:MM.>

<<<file: issues.md>>>
## <Short title of the error, bug or blocker>
<symptom, where seen, whether it was resolved in the capture. Every line
ends with HH:MM-HH:MM.>

<<<reasoning>>>
<2-4 sentences on how you clustered the threads and what you left out.>
```

**Rules:**

- A file with nothing to report is exactly the line `Nothing evident.`
  All three file markers must always appear.
- Every non-heading line carries a time range `HH:MM-HH:MM` from a heading
  in the input. A claim you cannot place in time is a claim to leave out.
- Distinguish doing (editing, committing, running) from seeing. Doing
  makes a thread; seeing at most a product mention.
- A block showing a record of an earlier day (a previous summary, an old
  log) is evidence that the user reviewed it today, never that the work
  happened today.
- Ignore interface chrome, counters and media state that survived
  filtering.
- At most 200 lines per file.

The date is {{DATE}}.

Timeline of the whole day:

{{TIMELINE}}

---

{{INPUT}}
````

`src-tauri/prompts/ingest-websites.md`:

````markdown
You are reading a table of every web page the user visited in one day,
merged by URL and ranked by time spent, and turning it into one short,
cited file about what they read with intent.

**How to read the input:**

- Columns: domain, title, dwell (minutes), visits, first seen, last seen,
  url. Dwell and visits are exact. Page bodies were not captured; the URL
  is the reference.
- Feed and social domains (x.com, youtube.com, reddit.com,
  news.ycombinator.com, linkedin.com) are browsing unless a single page
  held attention for a long time.
- The timeline below lists every block of the day so you can place a
  visit against the surrounding work.

**Produce exactly this output, with no code fence and nothing before the
first marker:**

```
<<<file: reading.md>>>
## <Topic>
- <title> · <domain> · <dwell>m · HH:MM-HH:MM · <url>

<<<reasoning>>>
<2-3 sentences on how you grouped topics and what you rolled up.>
```

**Rules:**

- Group by what the pages are about, not by domain. Roll feed browsing
  up to one line per domain: `- browsing · x.com · 34m · HH:MM-HH:MM`.
- Every entry line ends with a time range `HH:MM-HH:MM` built from the
  row's first and last seen.
- Rank topics by total dwell. Skip anything under two minutes unless it
  was revisited.
- Nothing to report is exactly the line `Nothing evident.`
- At most 200 lines.

The date is {{DATE}}.

Timeline of the whole day:

{{TIMELINE}}

---

{{INPUT}}
````

- [ ] **Step 2: Rewrite the input section of `day-context.md`**

Replace everything from the opening paragraph to the end of "How to read the input" with:

```markdown
You are turning one day of ambient screen capture into a compact context
document about the user's day. You are given two things: the day's
timeline (every block heading, in order) and a knowledge base that an
earlier pass built from the raw record. Your output will be read by other
LLMs (and occasionally the user) to understand what happened that day, so
precision and honesty matter more than completeness.

**How to read the input:**

- The timeline is the clock. Each `## HH:MM–HH:MM · App · Window title`
  line is one stretch of attention. Read it all before the knowledge base.
- The knowledge base is the evidence: `people.md`, `commitments.md`,
  `threads.md`, `products.md`, `issues.md`, `reading.md`. Every line in it
  already carries the time range and reference that supports it. Cite
  those; do not invent new ones.
- Raw bodies were left out on purpose. Where the knowledge base is silent,
  the timeline still tells you where the time went; say what the headings
  support and no more.
- `Nothing evident.` in a file means the earlier pass found nothing of
  that kind, not that capture was missing.
- Time is the strongest signal. A 40-minute block outweighs ten 30-second
  blocks. Repeated returns to the same thread matter more than any single
  visit.
```

Replace the tail (`The date is {{DATE}}. The captured day follows.` and `{{DAY_FILE}}`) with:

```markdown
The date is {{DATE}}.

Timeline:

{{TIMELINE}}

Knowledge base:

{{KB}}
```

Keep every `## ` heading in the output template and every rule unchanged.

- [ ] **Step 3: Failing tests in `prompt.rs`**

```rust
#[test]
fn every_bundled_prompt_passes_its_own_validation() {
    for id in PromptId::all() {
        validate(id, id.bundled()).unwrap_or_else(|e| panic!("{}: {e}", id.as_str()));
    }
}

#[test]
fn the_summary_prompt_needs_its_placeholders() {
    let text = PromptId::DayContext.bundled().replace("{{KB}}", "{{DAY_FILE}}");
    assert_eq!(validate(PromptId::DayContext, &text).unwrap_err(), PromptError::MissingPlaceholder("{{KB}}".into()));
}

#[test]
fn an_ingest_prompt_needs_its_file_markers() {
    let text = PromptId::IngestApps.bundled().replace("<<<file: issues.md>>>", "<<<file: problems.md>>>");
    assert_eq!(validate(PromptId::IngestApps, &text).unwrap_err(), PromptError::MissingMarker("<<<file: issues.md>>>".into()));
}

#[test]
fn prompts_are_customised_independently() {
    let dir = tempdir().unwrap();
    let mine = format!("{}\n\nExtra.\n", PromptId::IngestMessages.bundled());
    set(dir.path(), PromptId::IngestMessages, &mine).unwrap();
    assert!(is_customised(dir.path(), PromptId::IngestMessages));
    assert!(!is_customised(dir.path(), PromptId::DayContext));
    assert_eq!(prompt_path(dir.path(), PromptId::IngestMessages), dir.path().join("prompts").join("ingest-messages.md"));
    reset(dir.path(), PromptId::IngestMessages).unwrap();
    assert_eq!(current(dir.path(), PromptId::IngestMessages), PromptId::IngestMessages.bundled());
}

#[test]
fn ids_round_trip_through_strings() {
    for id in PromptId::all() {
        assert_eq!(PromptId::parse(id.as_str()), Some(id));
    }
    assert_eq!(PromptId::parse("nope"), None);
}
```

Keep the existing heading tests, calling `validate(PromptId::DayContext, ...)`.

- [ ] **Step 4: Run tests to verify they fail**

Run: `cd src-tauri && cargo test prompt::`
Expected: FAIL.

- [ ] **Step 5: Implement `prompt.rs`**

```rust
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PromptId {
    DayContext,
    IngestMessages,
    IngestApps,
    IngestWebsites,
}

pub const REQUIRED_HEADINGS: &[&str] = &[ /* unchanged */ ];

impl PromptId {
    pub fn all() -> [PromptId; 4] {
        [PromptId::DayContext, PromptId::IngestMessages, PromptId::IngestApps, PromptId::IngestWebsites]
    }
    pub fn as_str(self) -> &'static str {
        match self {
            PromptId::DayContext => "day-context",
            PromptId::IngestMessages => "ingest-messages",
            PromptId::IngestApps => "ingest-apps",
            PromptId::IngestWebsites => "ingest-websites",
        }
    }
    pub fn parse(name: &str) -> Option<PromptId> {
        PromptId::all().into_iter().find(|id| id.as_str() == name)
    }
    pub fn bundled(self) -> &'static str {
        match self {
            PromptId::DayContext => include_str!("../prompts/day-context.md"),
            PromptId::IngestMessages => include_str!("../prompts/ingest-messages.md"),
            PromptId::IngestApps => include_str!("../prompts/ingest-apps.md"),
            PromptId::IngestWebsites => include_str!("../prompts/ingest-websites.md"),
        }
    }
    pub fn placeholders(self) -> &'static [&'static str] {
        match self {
            PromptId::DayContext => &["{{DATE}}", "{{TIMELINE}}", "{{KB}}"],
            _ => &["{{DATE}}", "{{INPUT}}", "{{TIMELINE}}"],
        }
    }
    /// The file markers an ingest prompt must ask for, which are the files
    /// its call writes. Empty for the summary prompt.
    pub fn markers(self) -> &'static [&'static str] {
        match self {
            PromptId::DayContext => &[],
            PromptId::IngestMessages => &["<<<file: people.md>>>", "<<<file: commitments.md>>>"],
            PromptId::IngestApps => &["<<<file: threads.md>>>", "<<<file: products.md>>>", "<<<file: issues.md>>>"],
            PromptId::IngestWebsites => &["<<<file: reading.md>>>"],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptError {
    Empty,
    MissingHeading(String),
    MissingPlaceholder(String),
    MissingMarker(String),
    Io(String),
}
```

`Display`: `MissingPlaceholder(p) => "the prompt no longer contains {p}, which the app fills in"`, `MissingMarker(m) => "the prompt no longer asks for {m}, which its ingest call writes"`, `Io(e) => e`. Every function takes `id: PromptId`: `prompt_path(config_dir, id)` joins `format!("{}.md", id.as_str())`; `validate(id, text)` checks empty, then (DayContext only) headings, then placeholders, then markers; `set`/`reset`/`current`/`is_customised` are the existing bodies with the id threaded through and the `Io` variant used for write errors instead of `MissingHeading`.

- [ ] **Step 6: `summarise::build_prompt`**

```rust
pub fn build_prompt(template: &str, date: NaiveDate, timeline: &str, kb: &str) -> String {
    template
        .replace("{{DATE}}", &date.format("%Y-%m-%d").to_string())
        .replace("{{TIMELINE}}", timeline)
        .replace("{{KB}}", kb)
}
```

Remove `BUNDLED_PROMPT`. Existing `build_prompt` tests: assert both placeholders substituted and `{{DAY_FILE}}` no longer recognised (a template containing it comes back unchanged at that spot).

- [ ] **Step 7: Thread the id through `lib.rs`, `control.rs`, `mcp/files.rs`**

`PromptPayload` gains `id: String`. `prompt_payload(app, id)`. Commands:

```rust
fn prompt_id(id: Option<String>) -> Result<prompt::PromptId, String> {
    match id {
        None => Ok(prompt::PromptId::DayContext),
        Some(name) => prompt::PromptId::parse(&name).ok_or_else(|| format!("{name} is not a prompt id")),
    }
}

#[tauri::command]
fn get_prompt(app: tauri::AppHandle, id: Option<String>) -> Result<PromptPayload, String> { ... }
#[tauri::command]
fn set_prompt(app: tauri::AppHandle, id: Option<String>, text: String) -> Result<PromptPayload, String> { ... }
#[tauri::command]
fn reset_prompt(app: tauri::AppHandle, id: Option<String>) -> Result<PromptPayload, String> { ... }
```

Ledger entries for `set_prompt`/`reset_prompt` set `prompt_id: Some(id.as_str().to_string())`. `open_prompt_in_editor(app, id: Option<String>)` and `prompt_editor_target(config_dir, temp_dir, id)` name the read-only copy `Ambient Context {id} (bundled, read only).md`. `control.rs` `writes::set_prompt` and `mcp/files.rs::get_prompt` use `PromptId::DayContext` (MCP prompt tools keep operating on the summary prompt). `jobs.rs::run_one` uses `prompt::current(&config_dir, PromptId::DayContext)` until Task 10 replaces it.

Frontend `PromptSettings` passes `{ id: "day-context" }` for now; Task 12 adds the selector. `get_prompt` now returns a `Result`, so the mock in `AgentTab.test.tsx` keeps returning the payload object.

- [ ] **Step 8: Run everything**

Run: `cd src-tauri && cargo test && cd .. && npx tsc --noEmit && npx vitest run`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/prompts src-tauri/src/prompt.rs src-tauri/src/summarise.rs src-tauri/src/lib.rs src-tauri/src/control.rs src-tauri/src/mcp/files.rs src-tauri/src/jobs.rs src/components/PromptSettings.tsx src/test/AgentTab.test.tsx
git commit -m "Bundle three ingest prompts and address prompts by id"
```

---
### Task 9: `ingest.rs`: split, validate, trim, write, manifest

**Files:**
- Create: `src-tauri/src/ingest.rs`
- Modify: `src-tauri/src/lib.rs` (`mod ingest;`), `src-tauri/src/days.rs` (`has_kb: crate::ingest::has_kb(folder, date)`)

**Interfaces:**
- Consumes: `days::spans`, `days::timeline`, `summarise::unfence`, `ledger::sha256_of`, `prompt::PromptId`, `writer::DayFile`.
- Produces:
  - `ingest::Call { Messages, Apps, Websites }` with `ALL: [Call; 3]`, `action(self) -> &'static str` (`ingest_messages` etc.), `prompt(self) -> PromptId`, `source(self) -> DayFile` (for `Websites` the caller renders totals rather than reading the file), `files(self) -> &'static [&'static str]`, `label(self) -> &'static str` (`messages`, `apps`, `websites`).
  - `ingest::KB_FILES: [&str; 6]`, `ingest::MAX_KB_LINES: usize = 200`.
  - `ingest::kb_dir(folder, date) -> PathBuf`, `ingest::has_kb(folder, date) -> bool`, `ingest::read_kb(folder, date, file: Option<&str>) -> Option<String>`, `ingest::kb_for_prompt(folder, date) -> String`.
  - `ingest::Split { files: Vec<(String, String)>, reasoning: Option<String> }`, `ingest::split_output(text) -> Split`.
  - `ingest::Invalid` (Display), `ingest::validate(call, split, spans: &[(u32, u32)]) -> Result<(), Invalid>`.
  - `ingest::trim_input(text, max_chars) -> (String, usize)`.
  - `ingest::Frontmatter { date, source: String, generated_by: String, prompt_sha256: String }`, `ingest::write_call(folder, date, call, files: &[(String, String)], fm: &Frontmatter) -> io::Result<()>`, `ingest::write_skipped(folder, date, call) -> io::Result<()>`.
  - `ingest::Hashes { input, timeline, prompt }`, `ingest::CallRecord { disposition, input_sha256, timeline_sha256, prompt_sha256, engine, at }`, `ingest::Manifest { date, calls: BTreeMap<String, CallRecord> }`, `ingest::read_manifest(folder, date) -> Manifest`, `ingest::record_call(folder, date, call, record) -> io::Result<()>`, `ingest::needs_ingest(folder, date, call, hashes) -> bool`.

- [ ] **Step 1: Failing tests**

```rust
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
        assert_eq!(split.reasoning.as_deref(), Some("Dan wrote directly; newsletters were skipped."));
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
        assert_eq!(validate(Call::Messages, &split_output(&text), &spans()).unwrap_err(), Invalid::MissingFile("commitments.md".into()));
    }

    #[test]
    fn a_line_without_a_citation_is_rejected() {
        let text = GOOD_MESSAGES.replace(" 09:48-09:59 url: https://app.slack.com/x", "");
        assert!(matches!(validate(Call::Messages, &split_output(&text), &spans()).unwrap_err(), Invalid::NoCitation { file, .. } if file == "people.md"));
    }

    #[test]
    fn a_citation_outside_the_timeline_is_rejected() {
        let text = GOOD_MESSAGES.replace("09:48-09:59", "14:00-14:30");
        assert!(matches!(validate(Call::Messages, &split_output(&text), &spans()).unwrap_err(), Invalid::CitationOutsideTimeline { .. }));
    }

    #[test]
    fn nothing_evident_is_accepted_and_a_long_file_is_not() {
        let mut long = String::from("<<<file: reading.md>>>\n## Topic\n");
        for _ in 0..201 {
            long.push_str("- t · d · 3m · 09:00-09:30 · https://x\n");
        }
        assert!(matches!(validate(Call::Websites, &split_output(&long), &spans()).unwrap_err(), Invalid::TooLong { .. }));
        assert!(validate(Call::Websites, &split_output("<<<file: reading.md>>>\nNothing evident.\n"), &spans()).is_ok());
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
        let fm = Frontmatter { date: date(), source: "messages.md".into(), generated_by: "stub".into(), prompt_sha256: "abc".into() };
        let files = split_output(GOOD_MESSAGES).files;
        write_call(dir.path(), date(), Call::Messages, &files, &fm).unwrap();
        let people = std::fs::read_to_string(kb_dir(dir.path(), date()).join("people.md")).unwrap();
        assert!(people.starts_with("---\ndate: 2026-09-02\nkind: kb\nsource: messages.md\ngenerated_by: stub\nprompt_sha256: abc\n---\n\n## Dan"));
        assert!(!dir.path().join("KB").join(".tmp-2026-09-02-messages").exists());
        assert!(!has_kb(dir.path(), date()), "no accepted call recorded yet");
    }

    #[test]
    fn manifest_round_trips_and_drives_needs_ingest() {
        let dir = tempdir().unwrap();
        let hashes = Hashes { input: "i1".into(), timeline: "t1".into(), prompt: "p1".into() };
        assert!(needs_ingest(dir.path(), date(), Call::Apps, &hashes), "no manifest");
        record_call(dir.path(), date(), Call::Apps, CallRecord {
            disposition: "accepted".into(), input_sha256: "i1".into(), timeline_sha256: "t1".into(), prompt_sha256: "p1".into(), engine: "stub".into(), at: "2026-09-03T06:00:00+10:00".into(),
        }).unwrap();
        assert!(!needs_ingest(dir.path(), date(), Call::Apps, &hashes));
        assert!(needs_ingest(dir.path(), date(), Call::Messages, &hashes), "other call absent");
        assert!(needs_ingest(dir.path(), date(), Call::Apps, &Hashes { input: "i2".into(), ..hashes.clone() }), "input changed");
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
        let text = std::fs::read_to_string(kb_dir(dir.path(), date()).join("commitments.md")).unwrap();
        assert!(text.contains("source: none\n"));
        assert!(text.trim_end().ends_with("Nothing evident."));
        assert_eq!(read_manifest(dir.path(), date()).calls["ingest_messages"].disposition, "skipped");
    }

    #[test]
    fn kb_for_prompt_concatenates_without_frontmatter() {
        let dir = tempdir().unwrap();
        let fm = Frontmatter { date: date(), source: "messages.md".into(), generated_by: "stub".into(), prompt_sha256: "abc".into() };
        write_call(dir.path(), date(), Call::Messages, &split_output(GOOD_MESSAGES).files, &fm).unwrap();
        let out = kb_for_prompt(dir.path(), date());
        assert!(out.starts_with("# people.md\n\n## Dan"));
        assert!(out.contains("# commitments.md\n\n## I agreed to"));
        assert!(out.contains("# threads.md\n\n(not ingested)\n"));
        assert!(!out.contains("prompt_sha256"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test ingest::`
Expected: FAIL (module missing).

- [ ] **Step 3: Implement**

```rust
use crate::prompt::PromptId;
use crate::writer::DayFile;
use chrono::NaiveDate;
use regex::Regex;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const MAX_KB_LINES: usize = 200;
pub const KB_FILES: [&str; 6] = ["people.md", "commitments.md", "threads.md", "products.md", "issues.md", "reading.md"];
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

// ---- splitting ----

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
        reasoning: reasoning.map(|r| r.trim().to_string()).filter(|r| !r.is_empty()),
    }
}

// ---- validation ----

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invalid {
    Empty,
    MissingFile(String),
    DuplicateFile(String),
    UnexpectedFile(String),
    NoCitation { file: String, line: String },
    CitationOutsideTimeline { file: String, citation: String },
    TooLong { file: String, lines: usize, max: usize },
}

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Invalid::Empty => write!(f, "the agent returned nothing"),
            Invalid::MissingFile(name) => write!(f, "the output has no <<<file: {name}>>> section"),
            Invalid::DuplicateFile(name) => write!(f, "the output has two <<<file: {name}>>> sections"),
            Invalid::UnexpectedFile(name) => write!(f, "the output has a <<<file: {name}>>> section this call does not write"),
            Invalid::NoCitation { file, line } => write!(f, "{file}: a line carries no time range: {line:?}"),
            Invalid::CitationOutsideTimeline { file, citation } => write!(f, "{file}: {citation} is outside every captured block"),
            Invalid::TooLong { file, lines, max } => write!(f, "{file} is {lines} lines, over the {max} line budget"),
        }
    }
}

fn citation() -> &'static Regex {
    static CITATION: OnceLock<Regex> = OnceLock::new();
    CITATION.get_or_init(|| Regex::new(r"\b(\d{2}):(\d{2})[-\x{2013}](\d{2}):(\d{2})\b").unwrap())
}

fn inside(minute: u32, spans: &[(u32, u32)]) -> bool {
    spans.iter().any(|(s, e)| minute >= *s && minute <= *e)
        || spans.iter().any(|(s, e)| minute + 24 * 60 >= *s && minute + 24 * 60 <= *e)
}

pub fn validate(call: Call, split: &Split, spans: &[(u32, u32)]) -> Result<(), Invalid> {
    if split.files.is_empty() {
        return Err(Invalid::Empty);
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
            return Err(Invalid::MissingFile((*expected).to_string()));
        };
        let lines: Vec<&str> = body.lines().collect();
        if lines.len() > MAX_KB_LINES {
            return Err(Invalid::TooLong { file: (*expected).to_string(), lines: lines.len(), max: MAX_KB_LINES });
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
                return Err(Invalid::NoCitation { file: (*expected).to_string(), line: t.chars().take(80).collect() });
            };
            let n = |i: usize| caps[i].parse::<u32>().unwrap_or(0);
            let (start, end) = (n(1) * 60 + n(2), n(3) * 60 + n(4));
            if !inside(start, spans) || !inside(end, spans) {
                return Err(Invalid::CitationOutsideTimeline { file: (*expected).to_string(), citation: caps[0].to_string() });
            }
        }
    }
    Ok(())
}

// ---- trimming ----

/// Over `max_chars`, the longest block bodies are dropped first, headings
/// and reference lines kept. Returns the text and how many blocks lost
/// their body.
pub fn trim_input(text: &str, max_chars: usize) -> (String, usize) {
    if text.chars().count() <= max_chars {
        return (text.to_string(), 0);
    }
    // (heading and reference lines, body lines)
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
                if body.is_empty() && (line.is_empty() || line.starts_with("file: ") || line.starts_with("url: ") || line.starts_with("routed: ")) {
                    head.push(line.to_string());
                } else {
                    body.push(line.to_string());
                }
            }
        }
    }
    let mut trimmed = 0usize;
    let total = |blocks: &[(Vec<String>, Vec<String>)]| -> usize {
        preamble.iter().map(|l| l.chars().count() + 1).sum::<usize>()
            + blocks.iter().map(|(h, b)| h.iter().chain(b.iter()).map(|l| l.chars().count() + 1).sum::<usize>()).sum::<usize>()
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

// ---- writing ----

pub struct Frontmatter {
    pub date: NaiveDate,
    pub source: String,
    pub generated_by: String,
    pub prompt_sha256: String,
}

fn render_kb_file(fm: &Frontmatter, body: &str) -> String {
    format!(
        "---\ndate: {}\nkind: kb\nsource: {}\ngenerated_by: {}\nprompt_sha256: {}\n---\n\n{}\n",
        fm.date.format("%Y-%m-%d"), fm.source, fm.generated_by, fm.prompt_sha256, body.trim()
    )
}

pub fn write_call(folder: &Path, date: NaiveDate, call: Call, files: &[(String, String)], fm: &Frontmatter) -> std::io::Result<()> {
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
    let fm = Frontmatter { date, source: "none".into(), generated_by: "Ambient Context".into(), prompt_sha256: String::new() };
    let files: Vec<(String, String)> = call.files().iter().map(|n| ((*n).to_string(), NOTHING.to_string())).collect();
    write_call(folder, date, call, &files, &fm)?;
    record_call(folder, date, call, CallRecord {
        disposition: "skipped".into(), input_sha256: "none".into(), timeline_sha256: String::new(), prompt_sha256: String::new(),
        engine: String::new(), at: chrono::Local::now().to_rfc3339(),
    })
}

// ---- manifest ----

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

/// Flat dotted keys inside frontmatter: `ingest_apps.disposition: accepted`.
/// Still YAML, and parseable with a line loop.
pub fn read_manifest(folder: &Path, date: NaiveDate) -> Manifest {
    let mut manifest = Manifest::default();
    let Ok(text) = std::fs::read_to_string(manifest_path(folder, date)) else {
        return manifest;
    };
    for line in text.lines() {
        let Some((key, value)) = line.split_once(": ") else { continue };
        if key == "date" {
            manifest.date = value.trim().to_string();
            continue;
        }
        let Some((call, field)) = key.split_once('.') else { continue };
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

pub fn record_call(folder: &Path, date: NaiveDate, call: Call, record: CallRecord) -> std::io::Result<()> {
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
        "accepted" => record.input_sha256 != hashes.input || record.timeline_sha256 != hashes.timeline || record.prompt_sha256 != hashes.prompt,
        "skipped" => hashes.input != "none",
        _ => true,
    }
}

pub fn has_kb(folder: &Path, date: NaiveDate) -> bool {
    read_manifest(folder, date).calls.values().any(|r| r.disposition == "accepted")
}

// ---- reading ----

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

pub fn read_kb(folder: &Path, date: NaiveDate, file: Option<&str>) -> Option<String> {
    let dir = kb_dir(folder, date);
    match file {
        Some(name) if name == "manifest.md" || KB_FILES.contains(&name) => std::fs::read_to_string(dir.join(name)).ok(),
        Some(_) => None,
        None => {
            if !dir.is_dir() {
                return None;
            }
            Some(kb_for_prompt(folder, date))
        }
    }
}

/// The six files in a fixed order, each under a `# name` header, without
/// frontmatter. A file the ingest has not written yet says so.
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
```

Add `mod ingest;` in `lib.rs` and set `has_kb: crate::ingest::has_kb(folder, date)` in `days::entry`.

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test ingest:: days::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/ingest.rs src-tauri/src/lib.rs src-tauri/src/days.rs
git commit -m "Add the KB ingest module: split, validate, trim, write, manifest"
```

---
### Task 10: The pipeline in `jobs.rs`: three ingest calls, then the summary from the KB

**Files:**
- Modify: `src-tauri/src/jobs.rs` (`summarise_day` at line 165, `run_one` at line 231, `tick`, `JobStatus`/`JobSummary`/`QueuedJob`/`JobQueue`)
- Modify: `src-tauri/src/lib.rs` (`summarise_now`, `summarise_days`, `JobSummaryPayload`, `run_one` caller), `src-tauri/src/control.rs` (`summarise_day`)

**Interfaces:**
- Consumes: `ingest::*`, `days::timeline`, `days::spans`, `days::website_totals`, `days::render_totals`, `prompt::PromptId`, `prompt::current`, `summarise::build_prompt`, `agent::run_with_env`, `ledger::*`.
- Produces:
  - `jobs::Prompts { day_context, ingest_messages, ingest_apps, ingest_websites }` with `Prompts::load(config_dir)` and `for_call(&self, Call) -> &str`.
  - `jobs::Pipeline<'a> { folder: &Path, summary_agent: &Agent, ingest_agent: &Agent, prompts: &Prompts, ingest_max_chars: usize, reject_dir: &Path, env: &HashMap<String, String> }`.
  - `jobs::ingest_call(p: &Pipeline, call: Call, date, trigger: ledger::Trigger) -> Result<(), String>`.
  - `jobs::summarise_day(p: &Pipeline, date, trigger) -> Result<(), String>`.
  - `jobs::run_day_pipeline(p: &Pipeline, date, trigger, force_ingest: bool, summarise: bool, on_step: &mut dyn FnMut(&str)) -> Result<(), String>`.
  - `jobs::JobKind { Summarise, Ingest { force: bool } }` (Serialize, snake_case, `force` inline via `#[serde(tag = "kind")]`); `JobSummary.step: Option<String>`; `JobQueue::enqueue_ingest_with(date, force, trigger) -> JobId`; `JobQueue::record_step(id, step)`.
  - `run_one(app, date, trigger, kind: JobKind, on_step)`.

- [ ] **Step 1: Failing tests** (extend the `jobs.rs` test module; a stub agent is `/bin/sh -c 'cat > /dev/null; cat FILE'` so the reply is read from a fixture file)

```rust
fn stub_reply(dir: &std::path::Path, name: &str, reply: &str) -> crate::settings::Agent {
    let path = dir.join(name);
    std::fs::write(&path, reply).unwrap();
    stub_agent("/bin/sh", &["-c", &format!("cat > /dev/null; cat '{}'", path.display())])
}

fn write_days(folder: &std::path::Path) {
    use crate::writer::DayFile;
    let d = day(2026, 8, 28);
    for (file, text) in [
        (DayFile::Apps, "---\ndate: 2026-08-28\nkind: apps\n---\n\n## 09:00\u{2013}11:00 \u{00b7} Zed \u{00b7} jobs.rs\n\nfile: /x/jobs.rs\n\nfn tick\n\n## 11:00\u{2013}11:20 \u{00b7} Slack \u{00b7} #x\n\nrouted: messages\n"),
        (DayFile::Messages, "---\ndate: 2026-08-28\nkind: messages\n---\n\n## 11:00\u{2013}11:20 \u{00b7} Slack \u{00b7} #x\n\ndan: can you ship it thursday\n"),
        (DayFile::Websites, "---\ndate: 2026-08-28\nkind: websites\n---\n\n| start | end | app | domain | title | url |\n| --- | --- | --- | --- | --- | --- |\n| 09:30 | 09:41 | Arc | v2.tauri.app | Tauri | https://v2.tauri.app/ |\n"),
    ] {
        let path = file.path(folder, d);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

const APPS_REPLY: &str = "<<<file: threads.md>>>\n## jobs.rs\nEdited tick 09:00-11:00 file: /x/jobs.rs\n<<<file: products.md>>>\nNothing evident.\n<<<file: issues.md>>>\nNothing evident.\n<<<reasoning>>>\nOne thread.\n";
const MESSAGES_REPLY: &str = "<<<file: people.md>>>\n## Dan\nAsked to ship it Thursday 11:00-11:20\n<<<file: commitments.md>>>\n## I agreed to\n- [ ] ship it · with Dan · 11:00-11:20 · none\n\n## Owed to me\nNothing evident.\n";
const WEBSITES_REPLY: &str = "<<<file: reading.md>>>\nNothing evident.\n";

fn prompts() -> Prompts {
    Prompts {
        day_context: "{{DATE}}\n{{TIMELINE}}\n{{KB}}".into(),
        ingest_messages: "{{DATE}} {{TIMELINE}} {{INPUT}}".into(),
        ingest_apps: "{{DATE}} {{TIMELINE}} {{INPUT}}".into(),
        ingest_websites: "{{DATE}} {{TIMELINE}} {{INPUT}}".into(),
    }
}

#[test]
fn an_accepted_ingest_call_writes_its_files_manifest_and_ledger() {
    let folder = tempdir().unwrap();
    let rejects = tempdir().unwrap();
    write_days(folder.path());
    let agent = stub_reply(folder.path(), "reply.md", APPS_REPLY);
    let p = Pipeline { folder: folder.path(), summary_agent: &agent, ingest_agent: &agent, prompts: &prompts(), ingest_max_chars: 400_000, reject_dir: rejects.path(), env: &test_env() };
    ingest_call(&p, crate::ingest::Call::Apps, day(2026, 8, 28), crate::ledger::Trigger::OnDemand).unwrap();
    let kb = crate::ingest::kb_dir(folder.path(), day(2026, 8, 28));
    assert!(kb.join("threads.md").exists());
    assert!(std::fs::read_to_string(kb.join("products.md")).unwrap().contains("generated_by: stub"));
    assert_eq!(crate::ingest::read_manifest(folder.path(), day(2026, 8, 28)).calls["ingest_apps"].disposition, "accepted");
    let ledger = std::fs::read_to_string(crate::ledger::ledger_path(folder.path(), Local::now().date_naive())).unwrap();
    assert!(ledger.contains("ingest_apps"));
    assert!(ledger.contains("Days/2026-08-28/timeline"));
    assert!(ledger.contains("One thread."));
}

#[test]
fn a_rejected_ingest_call_keeps_the_output_and_writes_no_kb_file() {
    let folder = tempdir().unwrap();
    let rejects = tempdir().unwrap();
    write_days(folder.path());
    let agent = stub_reply(folder.path(), "reply.md", "<<<file: threads.md>>>\n## jobs.rs\nno citation here\n<<<file: products.md>>>\nNothing evident.\n<<<file: issues.md>>>\nNothing evident.\n");
    let p = Pipeline { folder: folder.path(), summary_agent: &agent, ingest_agent: &agent, prompts: &prompts(), ingest_max_chars: 400_000, reject_dir: rejects.path(), env: &test_env() };
    let error = ingest_call(&p, crate::ingest::Call::Apps, day(2026, 8, 28), crate::ledger::Trigger::OnDemand).unwrap_err();
    assert!(error.contains("no time range"), "{error}");
    assert!(rejects.path().join("2026-08-28-apps.md").exists());
    assert!(!crate::ingest::kb_dir(folder.path(), day(2026, 8, 28)).join("threads.md").exists());
    assert_eq!(crate::ingest::read_manifest(folder.path(), day(2026, 8, 28)).calls["ingest_apps"].disposition, "rejected");
}

#[test]
fn the_pipeline_runs_three_calls_then_the_summary_and_skips_what_is_current() {
    let folder = tempdir().unwrap();
    let rejects = tempdir().unwrap();
    write_days(folder.path());
    // One agent script answering by which file it is asked for: the prompt
    // carries the INPUT, and each input has a distinguishing line.
    let script = format!(
        "input=$(cat); case \"$input\" in *'dan: can you'*) cat '{m}';; *'fn tick'*) cat '{a}';; *'| domain |'*) cat '{w}';; *) cat '{s}';; esac",
        m = write_fixture(folder.path(), "m.md", MESSAGES_REPLY).display(),
        a = write_fixture(folder.path(), "a.md", APPS_REPLY).display(),
        w = write_fixture(folder.path(), "w.md", WEBSITES_REPLY).display(),
        s = write_fixture(folder.path(), "s.md", &valid_summary()).display(),
    );
    let agent = stub_agent("/bin/sh", &["-c", &script]);
    let p = Pipeline { folder: folder.path(), summary_agent: &agent, ingest_agent: &agent, prompts: &prompts(), ingest_max_chars: 400_000, reject_dir: rejects.path(), env: &test_env() };
    let mut steps: Vec<String> = Vec::new();
    run_day_pipeline(&p, day(2026, 8, 28), crate::ledger::Trigger::OnDemand, false, true, &mut |s| steps.push(s.to_string())).unwrap();
    assert_eq!(steps, vec!["ingesting messages (1 of 3)", "ingesting apps (2 of 3)", "ingesting websites (3 of 3)", "summarising"]);
    assert!(crate::summarise::summary_path(folder.path(), day(2026, 8, 28)).exists());
    let ledger = std::fs::read_to_string(crate::ledger::ledger_path(folder.path(), Local::now().date_naive())).unwrap();
    for action in ["ingest_messages", "ingest_apps", "ingest_websites", "summarise_day"] {
        assert!(ledger.contains(action), "{action}");
    }
    assert!(ledger.contains("KB/2026-08-28/people.md"), "summary inputs list KB files");

    // Nothing changed: a second run makes no ingest calls.
    steps.clear();
    run_day_pipeline(&p, day(2026, 8, 28), crate::ledger::Trigger::OnDemand, false, true, &mut |s| steps.push(s.to_string())).unwrap();
    assert_eq!(steps, vec!["summarising"]);

    // Force: all three again, no summary.
    steps.clear();
    run_day_pipeline(&p, day(2026, 8, 28), crate::ledger::Trigger::OnDemand, true, false, &mut |s| steps.push(s.to_string())).unwrap();
    assert_eq!(steps.len(), 3);
}

fn write_fixture(dir: &std::path::Path, name: &str, text: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn a_day_without_messages_skips_that_call_and_writes_nothing_evident() {
    let folder = tempdir().unwrap();
    let rejects = tempdir().unwrap();
    write_days(folder.path());
    std::fs::remove_file(crate::writer::DayFile::Messages.path(folder.path(), day(2026, 8, 28))).unwrap();
    let script = format!(
        "input=$(cat); case \"$input\" in *'fn tick'*) cat '{a}';; *) cat '{w}';; esac",
        a = write_fixture(folder.path(), "a.md", APPS_REPLY).display(),
        w = write_fixture(folder.path(), "w.md", WEBSITES_REPLY).display(),
    );
    let agent = stub_agent("/bin/sh", &["-c", &script]);
    let p = Pipeline { folder: folder.path(), summary_agent: &agent, ingest_agent: &agent, prompts: &prompts(), ingest_max_chars: 400_000, reject_dir: rejects.path(), env: &test_env() };
    let mut steps: Vec<String> = Vec::new();
    run_day_pipeline(&p, day(2026, 8, 28), crate::ledger::Trigger::OnDemand, false, false, &mut |s| steps.push(s.to_string())).unwrap();
    assert_eq!(steps, vec!["ingesting apps (2 of 3)", "ingesting websites (3 of 3)"]);
    let manifest = crate::ingest::read_manifest(folder.path(), day(2026, 8, 28));
    assert_eq!(manifest.calls["ingest_messages"].disposition, "skipped");
    assert!(std::fs::read_to_string(crate::ingest::kb_dir(folder.path(), day(2026, 8, 28)).join("people.md")).unwrap().contains("Nothing evident."));
}

#[test]
fn a_failed_ingest_stops_the_pipeline_before_the_summary() {
    let folder = tempdir().unwrap();
    let rejects = tempdir().unwrap();
    write_days(folder.path());
    let agent = stub_agent("/bin/sh", &["-c", "echo not logged in >&2; exit 1"]);
    let p = Pipeline { folder: folder.path(), summary_agent: &agent, ingest_agent: &agent, prompts: &prompts(), ingest_max_chars: 400_000, reject_dir: rejects.path(), env: &test_env() };
    let error = run_day_pipeline(&p, day(2026, 8, 28), crate::ledger::Trigger::Schedule, false, true, &mut |_| {}).unwrap_err();
    assert!(error.contains("not logged in"), "{error}");
    assert!(!crate::summarise::summary_path(folder.path(), day(2026, 8, 28)).exists());
}
```

Update the two existing `summarise_day` tests (`a_malformed_run_writes_one_ledger_entry_and_no_summary`, `an_agent_that_never_returns_an_answer_is_still_ledgered`) to build a `Pipeline` with `prompts()` and call `summarise_day(&p, date, trigger)` after `write_days`; their assertions are unchanged.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test jobs::`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
use crate::ingest::{self, Call};
use crate::prompt::PromptId;

pub struct Prompts {
    pub day_context: String,
    pub ingest_messages: String,
    pub ingest_apps: String,
    pub ingest_websites: String,
}

impl Prompts {
    pub fn load(config_dir: &Path) -> Prompts {
        Prompts {
            day_context: crate::prompt::current(config_dir, PromptId::DayContext),
            ingest_messages: crate::prompt::current(config_dir, PromptId::IngestMessages),
            ingest_apps: crate::prompt::current(config_dir, PromptId::IngestApps),
            ingest_websites: crate::prompt::current(config_dir, PromptId::IngestWebsites),
        }
    }
    pub fn for_call(&self, call: Call) -> &str {
        match call {
            Call::Messages => &self.ingest_messages,
            Call::Apps => &self.ingest_apps,
            Call::Websites => &self.ingest_websites,
        }
    }
}

pub struct Pipeline<'a> {
    pub folder: &'a Path,
    pub summary_agent: &'a settings::Agent,
    pub ingest_agent: &'a settings::Agent,
    pub prompts: &'a Prompts,
    pub ingest_max_chars: usize,
    pub reject_dir: &'a Path,
    pub env: &'a std::collections::HashMap<String, String>,
}

fn timeline_input(folder: &Path, date: NaiveDate) -> Result<(String, ledger::Input), String> {
    let timeline = crate::days::timeline(folder, date).ok_or_else(|| format!("there is no capture for {date}"))?;
    let input = ledger::Input {
        path: PathBuf::from(format!("Days/{}/timeline", date.format("%Y-%m-%d"))),
        sha256: ledger::sha256_of(timeline.as_bytes()),
    };
    Ok((timeline, input))
}

/// The raw text one call reads, or None when that day has no such file.
fn call_input(folder: &Path, date: NaiveDate, call: Call) -> Option<String> {
    match call {
        Call::Websites => {
            crate::days::read_day(folder, date, crate::writer::DayFile::Websites)?;
            Some(crate::days::render_totals(&crate::days::website_totals(folder, date)))
        }
        other => crate::days::read_day(folder, date, other.source()),
    }
}

pub fn ingest_call(p: &Pipeline, call: Call, date: NaiveDate, trigger: ledger::Trigger) -> Result<(), String> {
    let (timeline, timeline_input) = timeline_input(p.folder, date)?;
    let template = p.prompts.for_call(call);
    let prompt_sha = ledger::sha256_of(template.as_bytes());

    let Some(raw) = call_input(p.folder, date, call) else {
        ingest::write_skipped(p.folder, date, call).map_err(|e| e.to_string())?;
        return Ok(());
    };
    let input_sha = ledger::sha256_of(raw.as_bytes());
    let timeline_sha = timeline_input.sha256.clone();
    let (input, trimmed) = ingest::trim_input(&raw, p.ingest_max_chars);

    let mut entry = ledger::Entry {
        at: Local::now(),
        trigger,
        action: call.action().to_string(),
        prompt_id: Some(call.prompt().as_str().to_string()),
        prompt_sha256: Some(prompt_sha.clone()),
        engine: Some(p.ingest_agent.label.clone()),
        inputs: vec![
            ledger::Input { path: call.source().path(p.folder, date), sha256: input_sha.clone() },
            timeline_input,
        ],
        output: None,
        reasoning: if trimmed > 0 { Some(format!("input trimmed: {trimmed} blocks")) } else { None },
        disposition: ledger::Disposition::Accepted,
    };
    let record = |disposition: &str| ingest::CallRecord {
        disposition: disposition.to_string(),
        input_sha256: input_sha.clone(),
        timeline_sha256: timeline_sha.clone(),
        prompt_sha256: prompt_sha.clone(),
        engine: p.ingest_agent.label.clone(),
        at: Local::now().to_rfc3339(),
    };

    let prompt = template
        .replace("{{DATE}}", &date.format("%Y-%m-%d").to_string())
        .replace("{{TIMELINE}}", &timeline)
        .replace("{{INPUT}}", &input);

    let output = match agent::run_with_env(p.ingest_agent, &prompt, p.env) {
        Ok(output) => output,
        Err(error) => {
            let message = error.to_string();
            entry.disposition = ledger::Disposition::Failed { stderr: message.clone() };
            record_in_ledger(p.folder, &entry);
            let _ = ingest::record_call(p.folder, date, call, record("failed"));
            return Err(message);
        }
    };
    entry.output = Some(output.clone());
    let split = ingest::split_output(&output);
    if let Some(reasoning) = &split.reasoning {
        entry.reasoning = Some(match entry.reasoning.take() {
            Some(prefix) => format!("{prefix}\n\n{reasoning}"),
            None => reasoning.clone(),
        });
    }
    let spans = crate::days::spans(&timeline);
    if let Err(invalid) = ingest::validate(call, &split, &spans) {
        let _ = std::fs::create_dir_all(p.reject_dir);
        let _ = std::fs::write(p.reject_dir.join(format!("{}-{}.md", date.format("%Y-%m-%d"), call.label())), &output);
        entry.disposition = ledger::Disposition::Rejected { reason: invalid.to_string() };
        record_in_ledger(p.folder, &entry);
        let _ = ingest::record_call(p.folder, date, call, record("rejected"));
        return Err(format!("{invalid}; the output was kept for inspection"));
    }
    let fm = ingest::Frontmatter {
        date,
        source: call.source().file_name().to_string(),
        generated_by: p.ingest_agent.label.clone(),
        prompt_sha256: prompt_sha.clone(),
    };
    ingest::write_call(p.folder, date, call, &split.files, &fm).map_err(|e| format!("the KB could not be written: {e}"))?;
    ingest::record_call(p.folder, date, call, record("accepted")).map_err(|e| e.to_string())?;
    record_in_ledger(p.folder, &entry);
    Ok(())
}

pub fn summarise_day(p: &Pipeline, date: NaiveDate, trigger: ledger::Trigger) -> Result<(), String> {
    let (timeline, timeline_input) = timeline_input(p.folder, date)?;
    let kb = ingest::kb_for_prompt(p.folder, date);
    let mut inputs = vec![
        ledger::hash_file(&crate::writer::DayFile::Apps.path(p.folder, date)).map_err(|e| e.to_string())?,
        timeline_input,
    ];
    for name in ingest::KB_FILES {
        if let Ok(input) = ledger::hash_file(&ingest::kb_dir(p.folder, date).join(name)) {
            inputs.push(input);
        }
    }
    let template = &p.prompts.day_context;
    let mut entry = ledger::Entry {
        at: Local::now(),
        trigger,
        action: "summarise_day".to_string(),
        prompt_id: Some("day-context".to_string()),
        prompt_sha256: Some(ledger::sha256_of(template.as_bytes())),
        engine: Some(p.summary_agent.label.clone()),
        inputs,
        output: None,
        reasoning: None,
        disposition: ledger::Disposition::Accepted,
    };
    let prompt = summarise::build_prompt(template, date, &timeline, &kb);
    // ...the rest is the existing body from `agent::run_with_env` onward,
    // with `p.summary_agent`, `p.env`, `p.reject_dir` and `p.folder`.
}

pub fn run_day_pipeline(
    p: &Pipeline,
    date: NaiveDate,
    trigger: ledger::Trigger,
    force_ingest: bool,
    summarise: bool,
    on_step: &mut dyn FnMut(&str),
) -> Result<(), String> {
    let (timeline, _) = timeline_input(p.folder, date)?;
    let timeline_sha = ledger::sha256_of(timeline.as_bytes());
    for (index, call) in Call::ALL.iter().enumerate() {
        let hashes = ingest::Hashes {
            input: call_input(p.folder, date, *call).map(|t| ledger::sha256_of(t.as_bytes())).unwrap_or_else(|| "none".into()),
            timeline: timeline_sha.clone(),
            prompt: ledger::sha256_of(p.prompts.for_call(*call).as_bytes()),
        };
        if !force_ingest && !ingest::needs_ingest(p.folder, date, *call, &hashes) {
            continue;
        }
        if hashes.input != "none" {
            on_step(&format!("ingesting {} ({} of 3)", call.label(), index + 1));
        }
        ingest_call(p, *call, date, trigger.clone())?;
    }
    if summarise {
        on_step("summarising");
        summarise_day(p, date, trigger)?;
    }
    Ok(())
}
```

Note `write_skipped` records `input_sha256: none`, and `needs_ingest` for a `skipped` record returns true only when a file has since appeared, so a skipped call is not re-run every night and emits no step text.

- [ ] **Step 4: Job kinds, steps and `run_one`**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JobKind {
    Summarise,
    Ingest { force: bool },
}
```

`QueuedJob` gains `kind: JobKind`; `JobSummary` gains `kind: JobKind` and `step: Option<String>`. `push(date, kind, trigger)`; `enqueue_summarise_with(date, trigger)` pushes `JobKind::Summarise`; new `enqueue_ingest_with(date, force, trigger)` pushes `JobKind::Ingest { force }`; `record_step(&self, id, step: &str)` sets `step` on the history entry; `record(id, status)` clears `step` when the status is terminal.

`run_one(app, date, trigger, kind, on_step: &mut dyn FnMut(&str))`:

```rust
pub fn run_one(app: &AppHandle, date: NaiveDate, trigger: ledger::Trigger, kind: JobKind, on_step: &mut dyn FnMut(&str)) -> Result<(), String> {
    let config = settings::load(app);
    let folder = config.folder.clone().ok_or("no capture folder is set")?;
    let summary_agent = config.agent.clone().ok_or("no agent is connected")?;
    let ingest_agent = config.ingest_agent.clone().unwrap_or_else(|| summary_agent.clone());
    let config_dir = settings::config_dir(app);
    let prompts = Prompts::load(&config_dir);
    let reject_dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("rejected");
    let env = crate::agent_env(app);
    let p = Pipeline { folder: &folder, summary_agent: &summary_agent, ingest_agent: &ingest_agent, prompts: &prompts, ingest_max_chars: config.ingest_max_chars, reject_dir: &reject_dir, env: &env };
    match kind {
        JobKind::Summarise => run_day_pipeline(&p, date, trigger, false, true, on_step),
        JobKind::Ingest { force } => run_day_pipeline(&p, date, trigger, force, false, on_step),
    }
}
```

In `tick`, scheduled days call `run_one(app, date, Trigger::Schedule, JobKind::Summarise, &mut |_| {})`; queued jobs pass `job.kind` and a closure `&mut |step| app.state::<JobQueue>().record_step(&job.id, step)`. Outcome messages: `Summarised {date}` for `Summarise`, `Ingested {date}` for `Ingest`.

`lib.rs`: `JobSummaryPayload` gains `step: Option<String>`; `summarise_now` and `summarise_days` are unchanged in signature and enqueue `Summarise`. `control.rs::summarise_day` checks `crate::days::read_day(&folder, date, DayFile::Apps)`.

- [ ] **Step 5: Run everything**

Run: `cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/jobs.rs src-tauri/src/lib.rs src-tauri/src/control.rs
git commit -m "Run three ingest calls before the summary and summarise from the KB"
```

---
## PR 3: KB in the window, Settings and MCP

### Task 11: KB mode, Ingest and Re-ingest in the Day view

**Files:**
- Modify: `src-tauri/src/lib.rs` (`target_path`, new commands `read_kb`, `ingest_now`; `generate_handler!`)
- Create: `src/components/KbPane.tsx`, `src/test/KbPane.test.tsx`
- Modify: `src/lib/days.ts`, `src/components/DayView.tsx`, `src/components/DayHeader.tsx`, `src/test/DayView.test.tsx`

**Interfaces:**
- Consumes: `ingest::read_kb`, `ingest::KB_FILES`, `JobQueue::enqueue_ingest_with`, `JobSummary.step`.
- Produces: Tauri `read_kb(date, file?: string) -> Option<String>`, `ingest_now(date, force: bool) -> { job_id }`; `open_in_editor` `which: "kb"` opens `KB/{date}/threads.md`. Frontend: `KbFile` union of the six names plus `"manifest.md"`, `KbPane({ date, refreshKey })`, `JobState.step: string | null`, `DayHeader` props `onIngest(force: boolean)`, `hasKb`, `step`.

- [ ] **Step 1: Rust commands**

```rust
#[tauri::command]
fn read_kb(app: tauri::AppHandle, date: String, file: Option<String>) -> Option<String> {
    let folder = settings::load(&app).folder?;
    ingest::read_kb(&folder, parse_date(&date).ok()?, file.as_deref())
}

#[derive(Serialize)]
struct IngestNowPayload { job_id: String }

#[tauri::command]
fn ingest_now(app: tauri::AppHandle, date: String, force: bool) -> Result<IngestNowPayload, String> {
    let parsed = parse_date(&date)?;
    let config = settings::load(&app);
    if config.folder.is_none() { return Err("no capture folder is set".to_string()); }
    if config.agent.is_none() { return Err("no agent is connected".to_string()); }
    let id = app.state::<jobs::JobQueue>().enqueue_ingest_with(parsed, force, ledger::Trigger::OnDemand);
    Ok(IngestNowPayload { job_id: id.to_string() })
}
```

`target_path` gains `"kb" => Some(ingest::kb_dir(folder, date).join("threads.md"))`. Register both commands.

- [ ] **Step 2: Failing frontend tests**

`src/test/KbPane.test.tsx`:

```tsx
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { callsOf, mockInvoke } from "./tauri-mock";
import { KbPane } from "../components/KbPane";

vi.mock("@tauri-apps/api/core", async () => {
  const mock = await import("./tauri-mock");
  return { invoke: mock.invoke };
});

describe("KbPane", () => {
  afterEach(cleanup);

  it("shows each file on its own tab and the empty states", async () => {
    mockInvoke((command, args) => {
      if (command !== "read_kb") throw new Error(`unexpected command ${command}`);
      if (args?.file === "people.md") return "---\ndate: 2026-08-27\n---\n\n## Dan\nAsked for the notch state 09:48-09:59\n";
      if (args?.file === "reading.md") return "---\n---\n\nNothing evident.\n";
      return null;
    });
    render(<KbPane date="2026-08-27" refreshKey={0} />);
    expect(await screen.findByText("Dan")).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "Reading" }));
    expect(await screen.findByText("Nothing evident.")).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "Threads" }));
    expect(await screen.findByText("Not ingested yet.")).toBeTruthy();
    expect(callsOf("read_kb").some((c) => c.args?.file === "threads.md")).toBe(true);
  });
});
```

In `DayView.test.tsx`, `handler` gains `case "read_kb": return null;` and `case "ingest_now": return { job_id: "job-9" };`, and `job_state` returns `{ id: "job-9", date: todayIso(), status: "running", stderr: null, step: "ingesting apps (2 of 3)" }`. Add:

```tsx
it("queues an ingest and shows the step text", async () => {
  mockInvoke(handler(null));
  render(<DayView />);
  await waitFor(() => expect(countOf("list_days")).toBeGreaterThan(0));
  fireEvent.click(screen.getByRole("button", { name: "Ingest" }));
  await waitFor(() => expect(callsOf("ingest_now")[0]?.args?.force).toBe(false));
  expect(await screen.findByText("ingesting apps (2 of 3)")).toBeTruthy();
});
```

- [ ] **Step 3: Run to verify they fail**

Run: `npx vitest run src/test/KbPane.test.tsx src/test/DayView.test.tsx`
Expected: FAIL.

- [ ] **Step 4: `KbPane.tsx`**

```tsx
import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";

export const KB_FILES = ["people.md", "commitments.md", "threads.md", "products.md", "issues.md", "reading.md"] as const;
export type KbFile = (typeof KB_FILES)[number] | "manifest.md";

function label(file: KbFile): string {
  const stem = file.replace(/\.md$/, "");
  return stem.charAt(0).toUpperCase() + stem.slice(1);
}

function stripFrontmatter(text: string): string {
  if (!text.startsWith("---\n")) return text;
  const end = text.indexOf("\n---\n", 4);
  return end === -1 ? text : text.slice(end + 5).replace(/^\n+/, "");
}

/// Headings, task lines, bullets and paragraphs; the KB files use nothing
/// else.
function render(markdown: string): ReactNode[] {
  return markdown.split("\n").filter((line) => line.trim() !== "").map((line, index) => {
    if (line.startsWith("## ")) return <h3 key={index}>{line.slice(3)}</h3>;
    if (line.startsWith("- [ ] ") || line.startsWith("- [x] ")) return <p key={index} className="kb-task">{line.slice(6)}</p>;
    if (line.startsWith("- ")) return <p key={index} className="kb-item">{line.slice(2)}</p>;
    return <p key={index}>{line}</p>;
  });
}

export function KbPane({ date, refreshKey }: { date: string; refreshKey: number }) {
  const [file, setFile] = useState<KbFile>("people.md");
  const [text, setText] = useState<string | null | undefined>(undefined);

  useEffect(() => {
    let cancelled = false;
    setText(undefined);
    void invoke<string | null>("read_kb", { date, file }).then((next) => {
      if (!cancelled) setText(next);
    });
    return () => {
      cancelled = true;
    };
  }, [date, file, refreshKey]);

  return (
    <section className="kb-pane">
      <div className="segmented" role="tablist">
        {[...KB_FILES, "manifest.md" as const].map((name) => (
          <button key={name} type="button" role="tab" aria-selected={file === name}
            className={file === name ? "segment is-current" : "segment"} onClick={() => setFile(name)}>
            {label(name)}
          </button>
        ))}
      </div>
      <div className="kb-pane-scroll">
        {text === undefined ? null : text === null ? (
          <p className="pane-empty">Not ingested yet.</p>
        ) : file === "manifest.md" ? (
          <pre>{text}</pre>
        ) : (
          render(stripFrontmatter(text))
        )}
      </div>
    </section>
  );
}
```

CSS beside `.summary-pane`: `.kb-pane { display: flex; flex-direction: column; min-height: 0; }`, `.kb-pane-scroll { overflow: auto; padding: 8px 16px; }`, `.kb-task::before { content: "☐ "; }`, `.kb-item::before { content: "· "; }`.

- [ ] **Step 5: Wire `DayView` and `DayHeader`**

`src/lib/days.ts` `JobState`-shaped types live in `DayView.tsx`: add `step: string | null` to `JobState`. In `DayView`:

- `const [kbRefresh, setKbRefresh] = useState(0);`
- `onIngest = useCallback(async (force: boolean) => { setMode("kb"); const started = await invoke<{ job_id: string }>("ingest_now", { date: selected, force }); setJob({ id: started.job_id, date: selected, status: "queued", stderr: null, step: null }); }, [selected])` with the same try/catch shape as `onSummarise`.
- The `job_state` poll copies `state.step` into `job`; on `done` it also calls `setKbRefresh((n) => n + 1)`.
- Render `mode === "kb" ? <KbPane date={selected} refreshKey={kbRefresh} /> : ...`.
- Pass `hasKb={entry?.has_kb ?? false}`, `step={job && job.date === selected ? job.step : null}`, `onIngest={onIngest}` to `DayHeader`.

`DayHeader`: a third segment `KB` between Raw and Summary; buttons `Ingest` (calls `onIngest(false)`) and, when `hasKb`, `Re-ingest` (calls `onIngest(true)`); `summaryLine` shows `step` when the job is running and `step` is set, e.g. `running` case returns `step ?? "Summarising…"`. `onOpen` maps `kb` mode to `which: "kb"`.

- [ ] **Step 6: Run the frontend gates and commit**

Run: `npx tsc --noEmit && npx vitest run && cd src-tauri && cargo test`
Expected: PASS.

```bash
git add src-tauri/src/lib.rs src/lib/days.ts src/components/KbPane.tsx src/components/DayView.tsx src/components/DayHeader.tsx src/main-window.css src/test/KbPane.test.tsx src/test/DayView.test.tsx
git commit -m "Add the KB view with Ingest and Re-ingest to the Day view"
```

---

### Task 12: Ingest agent picker, prompt selector, input cap

**Files:**
- Modify: `src/components/AgentTab.tsx`, `src/components/PromptSettings.tsx`, `src/test/AgentTab.test.tsx`
- Modify: `src-tauri/src/lib.rs` (`open_prompt_in_editor(id)` already accepts an id from Task 8)

**Interfaces:**
- Consumes: `Settings.ingest_agent`, `Settings.ingest_max_chars`, Tauri `get_prompt(id)`, `set_prompt(id, text)`, `reset_prompt(id)`, `open_prompt_in_editor(id)`.
- Produces: `PromptSettings` renders a `<select id="prompt-id">` over `day-context`, `ingest-messages`, `ingest-apps`, `ingest-websites`; `AgentTab` renders `<select id="ingest-agent">` with `Same as summary` plus one option per detected agent (value = command) and `<input id="ingest-max-chars" type="number">`.

- [ ] **Step 1: Failing tests in `AgentTab.test.tsx`**

Extend the existing handler so `get_settings` returns `ingest_agent: null, ingest_max_chars: 400000` and `agent_detect` returns two agents (`Claude Code` at `/usr/local/bin/claude`, `opencode` at `/usr/local/bin/opencode`); `get_prompt` returns `{ id: args?.id ?? "day-context", text: "...", customised: false, path: "/p" }` where the text embeds the id so the test can see which prompt loaded. Add:

```tsx
it("saves a separate ingest agent and the input cap", async () => {
  mockInvoke(handler());
  render(<AgentTab />);
  const picker = (await screen.findByLabelText("Ingest agent")) as HTMLSelectElement;
  fireEvent.change(picker, { target: { value: "/usr/local/bin/opencode" } });
  await waitFor(() => expect(callsOf("set_settings").length).toBe(1));
  const next = callsOf("set_settings")[0].args?.next as { ingest_agent: { command: string } | null };
  expect(next.ingest_agent?.command).toBe("/usr/local/bin/opencode");
  const cap = screen.getByLabelText("Longest ingest input (characters)") as HTMLInputElement;
  fireEvent.change(cap, { target: { value: "250000" } });
  fireEvent.blur(cap);
  await waitFor(() => expect(callsOf("set_settings").length).toBe(2));
  expect((callsOf("set_settings")[1].args?.next as { ingest_max_chars: number }).ingest_max_chars).toBe(250000);
});

it("switches the prompt editor between the four prompts", async () => {
  mockInvoke(handler());
  render(<AgentTab />);
  const select = (await screen.findByLabelText("Prompt")) as HTMLSelectElement;
  fireEvent.change(select, { target: { value: "ingest-apps" } });
  await waitFor(() => expect(callsOf("get_prompt").some((c) => c.args?.id === "ingest-apps")).toBe(true));
});
```

- [ ] **Step 2: Run to verify they fail**

Run: `npx vitest run src/test/AgentTab.test.tsx`
Expected: FAIL.

- [ ] **Step 3: Implement**

`PromptSettings`: `const [id, setId] = useState<PromptId>("day-context")` where `export type PromptId = "day-context" | "ingest-messages" | "ingest-apps" | "ingest-websites"`; `read` calls `invoke<PromptPayload>("get_prompt", { id })` and depends on `id`; `save`/`reset`/`openInEditor` pass `{ id, ... }`. Above the textarea:

```tsx
<div className="field-row-stacked">
  <label htmlFor="prompt-id">Prompt</label>
  <select id="prompt-id" value={id} onChange={(event) => setId(event.target.value as PromptId)}>
    <option value="day-context">Daily summary</option>
    <option value="ingest-messages">Ingest messages</option>
    <option value="ingest-apps">Ingest apps</option>
    <option value="ingest-websites">Ingest websites</option>
  </select>
</div>
```

Legend becomes `Prompts`. The textarea id becomes `prompt-text`.

`AgentTab`: after the provider list, a fieldset `Ingest`:

```tsx
<fieldset>
  <legend>Ingest</legend>
  <p className="settings-note">
    Three shorter calls build the day's knowledge base before the summary. A cheaper agent can run them.
  </p>
  <div className="field-row-stacked">
    <label htmlFor="ingest-agent">Ingest agent</label>
    <select id="ingest-agent" value={settings.ingest_agent?.command ?? ""} disabled={!connected}
      onChange={(event) => {
        const found = detected.find((d) => d.agent.command === event.target.value)?.agent ?? null;
        void save((next) => ({ ...next, ingest_agent: found }));
      }}>
      <option value="">Same as summary</option>
      {detected.map((d) => (
        <option key={d.agent.command} value={d.agent.command}>{d.agent.label}</option>
      ))}
    </select>
  </div>
  <div className="field-row-stacked">
    <label htmlFor="ingest-max-chars">Longest ingest input (characters)</label>
    <input id="ingest-max-chars" type="number" min={10000} step={10000} value={capDraft}
      onChange={(event) => setCapDraft(event.target.value)}
      onBlur={() => { const n = Number(capDraft); if (Number.isFinite(n) && n >= 10000) void save((next) => ({ ...next, ingest_max_chars: n })); }} />
  </div>
</fieldset>
```

with `const [capDraft, setCapDraft] = useState("")` initialised from `settings.ingest_max_chars` once settings load. When the summary agent is disconnected (`agent: null`), also clear `ingest_agent`.

- [ ] **Step 4: Run the gates and commit**

Run: `npx tsc --noEmit && npx vitest run`
Expected: PASS.

```bash
git add src/components/AgentTab.tsx src/components/PromptSettings.tsx src/test/AgentTab.test.tsx
git commit -m "Add the ingest agent picker, input cap and prompt selector"
```

---

### Task 13: MCP `read_kb` and `ingest_day`

**Files:**
- Modify: `src-tauri/src/ipc.rs` (`Request::IngestDay { date, force, client }`)
- Modify: `src-tauri/src/control.rs` (`handle`, new `ingest_day`)
- Modify: `src-tauri/src/mcp/client.rs` (request mapping for `ingest_day`), `src-tauri/src/mcp/tools.rs` (two defs, `read_call` arm, `EXPECTED` becomes 20 names), `src-tauri/src/mcp/files.rs` (`read_kb`)
- Modify: `docs/mcp.md`

**Interfaces:**
- Produces: MCP `read_kb { date, file? }` (read-only, works with the app closed; `file` is one of the six KB names or `manifest.md`; absent returns the six concatenated); MCP `ingest_day { date, force? }` returning `{ job_id, status: "queued" }`; `summarise_day` docs state that ingest runs first.

- [ ] **Step 1: Failing tests**

In `tools.rs` tests, `EXPECTED` becomes 20 entries adding `"ingest_day"` and `"read_kb"` in sorted position, and the test name says twenty. Add:

```rust
#[test]
fn read_kb_returns_one_file_or_all_six() {
    let dir = tempfile::tempdir().unwrap();
    let date = chrono::NaiveDate::from_ymd_opt(2026, 8, 30).unwrap();
    let kb = crate::ingest::kb_dir(dir.path(), date);
    std::fs::create_dir_all(&kb).unwrap();
    std::fs::write(kb.join("people.md"), "---\n---\n\n## Dan\nx 09:00-09:10\n").unwrap();
    let mut server = server_with_folder(dir.path());
    let out = call(&mut server, "read_kb", &json!({ "date": "2026-08-30", "file": "people.md" }));
    assert!(out["content"][0]["text"].as_str().unwrap().contains("## Dan"));
    let out = call(&mut server, "read_kb", &json!({ "date": "2026-08-30" }));
    assert!(out["content"][0]["text"].as_str().unwrap().contains("# threads.md\n\n(not ingested)"));
    let out = call(&mut server, "read_kb", &json!({ "date": "2026-08-31" }));
    assert!(out["isError"].as_bool().unwrap_or(false));
}
```

In `control.rs` tests (beside the `summarise_day` one, if present) or `client.rs` tests: `ingest_day` with `{ "date": "2026-08-30", "force": true }` maps to `Request::IngestDay { date, force: true, client }`.

- [ ] **Step 2: Implement**

`ipc.rs`: `IngestDay { date: String, #[serde(default)] force: bool, client: String }`.

`control.rs`:

```rust
Request::IngestDay { date, force, client } => ingest_day(app, &date, force, &client),

fn ingest_day(app: &AppHandle, date: &str, force: bool, client: &str) -> Response {
    // Same checks as summarise_day, then:
    let id = queue.enqueue_ingest_with(date, force, ledger::Trigger::Mcp { client: client.to_string() });
    Response::Ok(serde_json::json!({ "job_id": id.to_string(), "status": "queued", "note": "Poll capture_status and look for this job id under jobs." }))
}
```

`mcp/client.rs`: `"ingest_day" => Request::IngestDay { date: date()?, force: arguments["force"].as_bool().unwrap_or(false), client }`, and add `ingest_day` to the list of socket-backed tool names.

`mcp/files.rs`:

```rust
pub fn read_kb(folder: &Path, date: NaiveDate, file: Option<&str>) -> Result<String, FileError> {
    crate::ingest::read_kb(folder, date, file).ok_or(FileError::NoKb(date))
}
```

with `FileError::NoKb(NaiveDate)` displaying `"There is no knowledge base for {date} yet. Call ingest_day to build one."`.

`tools.rs` defs:

```rust
Def {
    name: "read_kb",
    title: "Read a day's knowledge base",
    description: "Returns the structured notes the ingest step built for one day: people, commitments, threads, products, issues and reading, every line cited with a time range. Supply file to read one of them or the manifest; leave it out for all six. Works with the app closed.",
    input_schema: args(json!({
        "date": date_property(),
        "file": { "type": "string", "enum": ["people.md", "commitments.md", "threads.md", "products.md", "issues.md", "reading.md", "manifest.md"], "description": "One file, or leave out for all six." }
    }), &["date"]),
    read_only: true, destructive: false, idempotent: true,
},
Def {
    name: "ingest_day",
    title: "Build a day's knowledge base",
    description: "Queues the three ingest calls for one day (messages, apps, websites) without summarising. Calls whose inputs have not changed are skipped unless force is true. Returns a job id; poll capture_status. Needs Ambient Context running with an agent connected.",
    input_schema: args(json!({ "date": date_property(), "force": { "type": "boolean", "description": "Re-run every call even when nothing changed. Defaults to false." } }), &["date"]),
    read_only: false, destructive: true, idempotent: false,
},
```

`read_call` gains a `"read_kb"` arm mirroring `read_summary`. `summarise_day`'s description gains: "Runs the ingest calls first for anything out of date, then the summary."

`docs/mcp.md`: sections `### \`read_kb\`` and `### \`ingest_day\`` (after `summarise_day`), `summarise_day` prose updated, the tool count in the registration prose updated from eighteen to twenty, and `capture_status` jobs documented as carrying `kind` and `step`.

- [ ] **Step 3: Run tests and commit**

Run: `cd src-tauri && cargo test && cargo test --test docs_match_tools`
Expected: PASS.

```bash
git add src-tauri/src/ipc.rs src-tauri/src/control.rs src-tauri/src/mcp docs/mcp.md
git commit -m "Expose read_kb and ingest_day over MCP"
```

---

### Task 14: AGENTS.md, version 0.2.0, handover and manual QA

**Files:**
- Modify: `src-tauri/assets/AGENTS.md`, `src-tauri/src/writer.rs` (`PREVIOUS_BUNDLED_AGENTS`)
- Modify: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `package.json` (version `0.2.0`; `Cargo.lock` and `package-lock.json` follow from a build)
- Modify: `docs/handover.md`, `CHANGELOG.md`

- [ ] **Step 1: Record the current bundled AGENTS.md hash, then rewrite it**

Run `shasum -a 256 src-tauri/assets/AGENTS.md` before editing and append the hash to `PREVIOUS_BUNDLED_AGENTS`, so an untouched 0.1 copy in the capture folder is replaced rather than left beside a `.new`. Then rewrite the file with these sections, keeping the tone of the current one:

- `# Reading this folder`: what the app is, that the reader is an LLM.
- `## Files`: `Days/YYYY-MM-DD/apps.md` (the timeline; native app bodies), `websites.md` (visit table, no bodies), `messages.md` (message bodies); `KB/YYYY-MM-DD/` (six cited files plus `manifest.md`); `Summaries/`; `Ledger/`; `AGENTS.md`.
- `## Format of the day files`: the block format with `routed:` lines, the websites table columns.
- `## The knowledge base`: derived, regenerable, every line cited with `HH:MM-HH:MM`, `Nothing evident.` semantics, `manifest.md` records which call produced what and from which input hashes.
- `## How to read it`: summary for what a day meant, KB for the structured evidence, `Days/` for the record; headings are the timeline; a bare heading is a return, not nothing; follow `file:`/`url:` references; `[redacted]`; uncaptured hours mean not recorded.
- `## Ledger`: as now, plus the three `ingest_*` actions.

Add a writer test asserting `ensure_agents_file` replaces a file whose content hashes to the newly added entry.

- [ ] **Step 2: Bump the version**

`version = "0.2.0"` in `src-tauri/Cargo.toml`, `"version": "0.2.0"` in `src-tauri/tauri.conf.json` and `package.json`. Run `cd src-tauri && cargo build` and `npm install --package-lock-only` so the lock files update. `CHANGELOG.md` gains a `## 0.2.0` entry listing: Days/ layout, website visit rows, message routing and `route_messages`, own-window headings-only, KB with three ingest calls, summary from KB, KB view, ingest agent, `read_kb` and `ingest_day` MCP tools, and that 0.1 flat day files are no longer read.

- [ ] **Step 3: Handover**

In `docs/handover.md`: under "Built and merged" describe this feature in one paragraph with the spec and plan paths; remove the "day-scoped knowledge base" entry from "Specced and planned, not built"; update "What the app is" (one folder per day, three files, a knowledge base, then a summary); update the CI test counts.

- [ ] **Step 4: Manual QA** (record the outcome in the handover)

1. Build and run: `npm run tauri dev`. Set the capture folder to a fresh test folder.
2. Capture a mixed hour: Zed on a source file, Arc on the Tauri docs, `x.com/home`, `x.com/messages`, Mail, Slack. Confirm `Days/{today}/apps.md` has every heading, `websites.md` has rows for the Arc and `x.com/home` visits with no bodies, `messages.md` has the Mail, Slack and `x.com/messages` bodies with no `7:09 am` style timestamps.
3. Open `Days/{today}/apps.md` in Zed while capture runs; confirm no block for it appears. Confirm the Ambient Context window appears as headings only.
4. Day view: Raw tabs switch; Websites tab ranks by dwell.
5. Press Ingest. Watch the step text advance through 1 of 3 to 3 of 3. Inspect all six KB files: frontmatter written by Rust, every line cited, `Nothing evident.` where empty. Open `manifest.md`.
6. Press Summarise. Confirm the ledger for today lists four entries, the summary entry's inputs include the six KB paths and `apps.md`, and the summary cites times that exist in the timeline.
7. Delete `KB/{today}/`, press Re-ingest, confirm regeneration and that `has_kb` returns.
8. Set a different ingest agent in the Agent tab, re-ingest, confirm `generated_by` in the KB files names it.
9. Over MCP: `read_day` with `file: "websites"`, `read_kb`, `ingest_day`, `summarise_day`.

- [ ] **Step 5: Full CI locally, then commit**

Run all gates from Global Constraints.

```bash
git add src-tauri/assets/AGENTS.md src-tauri/src/writer.rs src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json package.json package-lock.json docs/handover.md CHANGELOG.md
git commit -m "Ship 0.2.0: per-day folders, daily KB and the handover for it"
```

---

## Out of Scope Reminder

- Persistent cross-day wiki or entity merge.
- Replay marker and its prompt rule.
- Citation and token validators on the summary output.
- Migrating 0.1 flat day files.
- Meta description or any HTTP fetch from the recorder.
- Changing the segmenter, `max_block_chars` default, or `normalise_line`.
