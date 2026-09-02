# Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the items from the 2 September product review that improve every summary from now on: idle detection, the replay marker, a failure notification, and ten small fixes.

**Architecture:** Each task is self-contained and touches a disjoint file set from the other tasks in its wave. Wave 1: idle detection (Swift, reader, capture, settings) and the writer-side changes (replay marker, tracking-parameter stripping). Wave 2: summary validation, and the settings and lib.rs polish. Wave 3: ledger duration, rejected-write ledgering and the notification (jobs, ledger, plugin). The signed-build work runs in parallel in the same tree and owns `tauri.conf.json`, `.github/` and anything signing-related; no task here touches those.

**Tech Stack:** Rust (Tauri 2, chrono, regex), Swift via swift-rs, React 19, TypeScript, Vitest.

**Source:** `2. Ideas/Apps/Ambient Context/Reviews/Review - Product 2026-09-02.md` in the vault (items 5, 7, 8 and the Small improvements table).

## Global Constraints

- Australian English in prose, comments and UI copy. NEVER an em-dash (U+2014) anywhere, including commit messages; a git hook blocks them. Use a comma, colon, parentheses or two sentences.
- Another person is editing this working tree at the same time (release signing). `git add` only the files your task names. Never `git add -A`, never stash, never touch `tauri.conf.json`, `.github/`, `Info.plist`, `entitlements`, or the updater config.
- TDD: write the failing test, run it, implement, run it, then the full gate.
- Gate, from the repo root, before every commit: `export PATH="$HOME/.cargo/bin:$PATH"; cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && cd .. && npx tsc --noEmit && npx vitest run && npm run build`.
- No `@testing-library/jest-dom`. Tauri is mocked through `src/test/tauri-mock.ts`; every frontend test names the commands it expects.
- Commit messages: imperative, no `Co-Authored-By`, no "Generated with" footer.
- `Days/` files are append-only. Nothing here rewrites them.

---

## Wave 1

### Task 1: Idle detection

**Files:**
- Modify: `src-tauri/plugins/ax/macos/Sources/AxPlugin.swift` (new export beside `ambient_ax_permission_status`)
- Modify: `src-tauri/src/reader/macos.rs`, `src-tauri/src/reader/windows.rs`, `src-tauri/src/reader/mod.rs`
- Modify: `src-tauri/src/capture.rs` (poll loop, around the `reader::PlatformReader.snapshot()` match)
- Modify: `src-tauri/src/settings.rs` (`idle_secs`; also change `max_block_chars` default from `0` to `4000`)
- Modify: `src/lib/days.ts` (`Settings.idle_secs`), `src/components/RecordingSettings.tsx`, `src/test/AppSettings.test.tsx` or the RecordingSettings test if one exists

**Behaviour:**
- Swift: `@_cdecl("ambient_ax_seconds_since_input") public func ambientAxSecondsSinceInput() -> Double` returning `CGEventSource.secondsSinceLastEventType(.combinedSessionState, eventType: .init(rawValue: ~0)!)` (the "any event" type, `kCGAnyInputEventType`). Return `-1` if the value cannot be read.
- `reader::WindowReader` gains `fn seconds_since_input(&self) -> Option<f64>`; macOS binds the Swift call (`None` when it returns below zero); Windows returns `None`.
- `Settings.idle_secs: u64`, default `120`, `serde(default)`. `0` disables the check.
- `capture.rs`: before taking a snapshot, if `idle_secs > 0` and `seconds_since_input()` is `Some(s)` with `s >= idle_secs as f64`, treat the poll as idle: flush the segmenter's open block (as the three-failed-reads path does), do not take a snapshot, and skip to the sleep. Log the transition to idle once (not every poll) with `eprintln!("[capture] idle after {s:.0}s, block closed")`, and log once when input resumes. Pure logic (`fn is_idle(idle_secs: u64, since: Option<f64>) -> bool`) unit-tested; the flush path is exercised by the existing `Segmenter::flush` tests.
- `max_block_chars` default becomes `4000`. Update the settings test that asserts defaults, and the `RecordingSettings.tsx` help copy ("0 is unlimited" stays true).
- UI: `RecordingSettings.tsx` gains a number field "Idle after (seconds)" with help text "When there has been no keyboard or mouse input for this long, the open block is closed and nothing is recorded until you return. 0 turns this off." Bound like the other numeric fields there.
- `docs/mcp.md` `set_config`: add `idle_secs` to the settable keys if the doc lists them, and `get_config` output if it enumerates fields. `files.rs::get_config` settable-keys list gains `idle_secs`.

**Tests:** `settings::tests` default check; `capture::tests::is_idle` cases (`0` disables, `None` is not idle, `119.9` under `120` is not, `120` is); a `reader::tests` check that `permission_from_code` is untouched (no change); frontend test that the field renders with the loaded value and saves `idle_secs` on blur.

**Commit:** `Close the open block and stop recording after idle input`

### Task 2: Replay marker, tracking parameters, fixture

**Files:**
- Modify: `src-tauri/src/writer.rs` (`append_block`, `render_block`, `render_website_row`)
- Create: `src-tauri/src/replay.rs` (detection), add `mod replay;` in `lib.rs`
- Modify: `src-tauri/src/days.rs` (`RawBlock.replay: Option<String>`, `parse_heading` tolerant of the marker, fixture at line ~422 reading `0.3.0` becomes `0.2.0`)
- Modify: `src-tauri/prompts/day-context.md` and `src-tauri/prompts/ingest-apps.md` (one rule each)
- Modify: `src/lib/rules.ts` (`RawBlock.replay`), `src/components/RawPane.tsx` (show a `replay: 2026-08-28` chip on the block heading), `src/test/RawPane.test.tsx`

**Behaviour:**
- `replay::detect(folder: &Path, block: &Block) -> Option<NaiveDate>`:
  1. If `block.document` or `block.url` contains `/Summaries/YYYY-MM-DD.md` or `/KB/YYYY-MM-DD/` where the date differs from `block.start.date_naive()`, return that date.
  2. Otherwise, for each of the previous seven days that has `Summaries/YYYY-MM-DD.md`, count how many of the block's non-empty lines (after trimming) appear verbatim as a line in that summary; if the count is more than half of the block's lines and at least three, return that date. Take the most recent match. Read each summary once per call; seven small files is cheap enough at block close, and this runs only for blocks with at least three lines.
- `writer::append_block`: when `detect` returns `Some(date)`, the heading in `apps.md` gains the suffix ` [replay: YYYY-MM-DD]` after the title, the body is dropped (as `headings_only`), and no lines enter the dedup set. Website and Message routing still apply to the heading placement (a replayed block is by construction an App block: it is a file in an editor, but keep the routing call so the code path is uniform).
- `days::parse_heading` accepts the suffix: strip a trailing ` [replay: YYYY-MM-DD]` from the title into `RawBlock.replay`. `timeline` keeps the marker in the heading text so the prompts see it.
- `render_website_row`: strip query parameters named `utm_*`, `fbclid`, `gclid`, `ref`, `ref_src`, `igshid` from the URL before writing; drop the `?` if nothing remains. Pure `fn strip_tracking(url: &str) -> String`, tested.
- Prompts: `day-context.md` under "How to read the input" gains: "A heading marked `[replay: DATE]` is a record of an earlier day being reviewed. Anything about it is evidence that the user looked at it today, never that the work happened today." `ingest-apps.md` gains the same sentence under its reading rules.
- `RawPane.tsx`: when `block.replay` is set, render `<span className="raw-replay">replay: {block.replay}</span>` beside the app name.

**Tests:** `replay::tests` (path rule for Summaries and KB, other-date only; line-overlap rule with a fixture summary, under-half not matched, fewer than three lines not matched); `writer::tests` (a replayed block writes the marked heading and no body, and its lines are absent from dedup); `days::tests` (`parse_heading` round-trips the marker, `strip_tracking` cases including a URL with only tracking params and one with a fragment); `RawPane.test.tsx` shows the chip.

**Commit:** `Mark replayed records at block close and strip tracking parameters from visit rows`

---

## Wave 2

### Task 3: Summary validation against the timeline

**Files:**
- Modify: `src-tauri/src/summarise.rs` (`validate` signature and body)
- Modify: `src-tauri/src/jobs.rs` (`summarise_day` call site only)
- Modify: `src-tauri/src/ingest.rs` only if the span check is extracted to share (prefer extracting `pub fn citation_in_spans(text: &str, spans: &[(u32, u32)]) -> Result<(), String>` into a new `src-tauri/src/cite.rs` used by both)

**Behaviour:**
- `summarise::validate(text, max_lines, spans: &[(u32, u32)], evidence: &str) -> Result<(), Invalid>`. Two new checks after the existing ones:
  1. Every `HH:MM-HH:MM` citation in the body (existing regex, en-dash or hyphen) has both endpoints inside some span (reuse `ingest::inside`, moved to `cite.rs`). Failure: `Invalid::CitationOutsideTimeline(String)`.
  2. Every token in the body matching `\b\d{3,}\b` (three or more digits, not part of a time) or `\b[0-9a-f]{7,40}\b` (hashes) appears somewhere in `evidence`, which the caller passes as the KB text plus the timeline. Failure: `Invalid::UnsupportedFigure(String)`. Times (`\d{1,2}:\d{2}`) and the frontmatter date are excluded from this check.
- `jobs::summarise_day` passes `days::spans(&timeline)` and `format!("{timeline}\n{kb}")`.

**Tests:** `summarise::tests`: the existing `good()` fixture still passes with a matching span; a citation outside every span is rejected naming it; a summary quoting "303 tests" with no `303` in the evidence is rejected naming the figure; a figure present in the evidence passes; times are not treated as figures. `jobs` pipeline test still passes (its summary fixture cites `09:00-11:00`, which is in the fixture timeline).

**Commit:** `Reject summaries whose citations or figures are not in the evidence`

### Task 4: Settings and lib.rs polish

**Files:**
- Modify: `src-tauri/src/lib.rs` (`set_settings`, startup autostart sync)
- Modify: `src-tauri/src/redact.rs` (`compile_extra` returns errors)
- Modify: `src/components/RecordingSettings.tsx` (redaction pattern error display), `src/components/AppSettings.tsx` (editor field), `src/test/AppSettings.test.tsx`

**Behaviour:**
- `redact::validate_extra(patterns: &[String]) -> Vec<(usize, String)>` returning the index and regex error of each invalid pattern. `set_settings` rejects a save whose `extra_redaction_patterns` has an invalid entry with the message `pattern {n} is not a valid regular expression: {error}`; the existing lenient `compile_extra` stays for capture. `RecordingSettings.tsx` shows the error under the textarea and does not clear the draft.
- `AppSettings.tsx` gains "Open files with" : a text field for an application path (`editor`), help text "The application used by every Open in editor button. Leave empty for the system default for markdown.", saved on blur, plus a "Choose…" button that calls a new command `choose_editor` using the dialog plugin (already a dependency) to pick an `.app`, returning its path.
- Startup autostart sync: when `manager.enable()`/`disable()` fails, `eprintln!` the error and set a flag on a new `AutostartState(Mutex<Option<String>>)` managed state; `get_settings` result is unchanged but a new command `autostart_error() -> Option<String>` returns it, and `AppSettings.tsx` shows "Login item could not be updated: {error}" under the checkbox when set.

**Tests:** `redact::tests::validate_extra_names_the_bad_pattern`; `lib.rs` unit test for the settings rejection message if `set_settings` logic is extracted to a pure `fn check_settings(&Settings) -> Result<(), String>` (do that); `AppSettings.test.tsx`: editor field renders the loaded value and saves `editor`; `autostart_error` text shows when the command returns one.

**Commit:** `Validate redaction patterns on save, expose the editor setting, surface login item errors`

---

## Wave 3

### Task 5: Ledger duration, rejected-write ledgering, failure notification

**Files:**
- Modify: `src-tauri/src/ledger.rs` (`Entry.took_ms: Option<u64>`, rendered as `- took: 4m 12s`)
- Modify: `src-tauri/src/jobs.rs` (`ingest_call`, `summarise_day`, `tick`)
- Modify: `src-tauri/Cargo.toml` (`tauri-plugin-notification = "2"`), `package.json` (`@tauri-apps/plugin-notification` not needed; notifications are sent from Rust), `src-tauri/capabilities/default.json` (`notification:default`), `src-tauri/src/lib.rs` (plugin init)
- Modify: `src-tauri/src/lib.rs` (`summarise_now` gains `force: Option<bool>`; plugin init)
- Does not touch `src/`: Task 6 owns every Day view file and renders `took_ms` and the step text.

**Behaviour:**
- Every agent-backed ledger entry records `took_ms` measured around `agent::run_with_env`. `ledger::render` writes `- took: {m}m {s}s` when present.
- `jobs::Outcome` gains `took_ms: Option<u64>` (the whole pipeline for that day), serialised so the window can show it.
- `JobKind::Summarise` becomes `Summarise { force: bool }`. `run_one` maps it to `run_day_pipeline(p, date, trigger, force, true, ...)`. `summarise_now(date, force: Option<bool>)` enqueues with `force.unwrap_or(false)`; `summarise_days` and the scheduled path enqueue `force: false`. MCP `summarise_day` gains an optional `force` argument documented in `docs/mcp.md`.
- Step text emitted by `run_day_pipeline` becomes user-facing copy: `Reading messages (1 of 3)`, `Reading apps (2 of 3)`, `Reading websites (3 of 3)`, `Writing the summary`. Update the jobs tests that assert the old strings.
- In `ingest_call` and `summarise_day`, a failure to create the reject directory or write the rejected output is appended to the ledger entry's `disposition` reason: `...; the output could not be kept: {error}`. No more `let _ =` on those two lines.
- Scheduled failures notify: in `tick`, on the scheduled path only, when `run_one` returns `Err`, send a macOS notification titled "Ambient Context" with body `{date}'s summary failed: {first line of the message}` via `tauri_plugin_notification::NotificationExt`. On-demand failures do not notify (the user is looking). Register the plugin in the builder, add the capability. If the notification send fails, `eprintln!` only.

**Tests:** `ledger::tests` renders `took`; `jobs` tests assert `took_ms` is `Some` on a stub-agent run, that a reject-dir failure (point `reject_dir` at a path under a file) produces a disposition containing "could not be kept", and that `Summarise { force: true }` re-runs an accepted call. The notification itself is not unit-testable; the manual check is a scheduled run with the agent logged out.

**Commit:** `Record run durations, ledger lost rejected output, notify on a failed scheduled summary`

### Task 6: One action and one status line in the Day view

Runs after Tasks 2, 4 and 5. Held by one agent: the view is judged as a whole.

**Files:**
- Modify: `src/components/DayView.tsx`, `src/components/DayHeader.tsx`, `src/components/KbPane.tsx` (rename to `NotesPane.tsx`), `src/components/SummaryPane.tsx` (empty-state copy), `src/main-window.css`
- Modify: `src/test/DayView.test.tsx`, `src/test/KbPane.test.tsx` (rename to `NotesPane.test.tsx`)
- Modify: `src-tauri/src/lib.rs` only for `read_kb` to also return the manifest's build time if that is simpler than parsing it in TypeScript (optional; parsing `manifest.md` lines `ingest_*.at:` in TS is fine)

**Behaviour:**
- **One action.** Header buttons: `Summarise` when the day has no summary, `Regenerate` when it has one; `Open in editor`; `Reveal in Finder`. Remove Ingest and Re-ingest and `onIngest`. `Regenerate` calls `summarise_now` with `force: true`; `Summarise` with `force: false`. The `ingest_now` command stays in the backend (MCP uses the same path) but nothing in the window calls it.
- **One status line** replaces the stats span and the summary-state span: `{hours} h recorded · {blocks} blocks · Summary {HH:MM}, took {m} min` when a summary exists (`took` from `Outcome.took_ms` when present), `{hours} h recorded · {blocks} blocks · No summary yet` otherwise. During a run the line is the current step from `job_state.step` (already user-facing after Task 5) with an ellipsis, or `Queued…`. On failure: `Last run failed: {reason}` in the existing error style, still one line plus the `<pre>` for the reason.
- **Modes renamed:** `Record`, `Notes`, `Summary`. Record keeps its Apps / Websites / Messages tabs. No "KB", "ingest" or "raw" appears in any visible string.
- **Notes is one page.** `NotesPane` reads the six files with one `read_kb` call each (or a single call with no `file`, then splits on the `# name.md` headers the backend writes; use the single call) and renders them as sections in the order people, commitments, threads, products, issues, reading, each with an `h2` label (People, Commitments, Threads, Products, Issues, Reading) and the file's body through the existing line renderer, or `Nothing evident` in the muted style when the body is that sentinel. Top line: `Built {HH:MM} from messages, apps and websites` from the manifest's latest `at:` value, listing only the calls whose disposition is `accepted`; `Not built yet. Summarise to build it.` with a Summarise button when there is no manifest. No tabs, no manifest view.
- **Empty states name the action.** Summary pane with no summary: one sentence and a `Summarise` button (already there; align the copy with the status line). Notes with nothing: as above.
- **Default mode** unchanged: today opens on Record; a past day opens on Summary if one exists, else Record.
- **Polling:** the job poll copies `step` into state on every tick; on `done` it reloads the day, the summary and the notes (bump `refreshKey`). Keep the existing guard against unbounded `read_day` loops; the existing test for it must still pass.
- CSS: `.notes-pane` takes `.kb-pane`'s rules; section headings use the existing heading style in the summary pane; the status line uses the `day-stats` style.

**Tests:** `DayView.test.tsx`: `Summarise` calls `summarise_now` with `force: false`; after a `job_status` with a summary, the button reads `Regenerate` and calls with `force: true`; the status line shows the step text while running; the three mode tabs are labelled Record, Notes, Summary; no element contains the text "Ingest". `NotesPane.test.tsx`: six sections render from one `read_kb` response, `Nothing evident` shows for an empty section, the built line names the accepted calls, the not-built state shows the button.

**Commit:** `Make Summarise the one action and lay out the notes as one page`

**Manual check (the user, in the running app):** open today; the status line reads correctly; press Summarise and watch the four steps; Notes renders as one page; Regenerate re-runs; nothing in the window says KB or ingest.

---

## Done outside the waves

- GitHub Issues enabled on `dragthelake/ambient-context` and a bug-report flow written to `docs/bug-reports.md` (what to include: app version from About, the day's ledger entry, the `rejected/` file if any, never the day files themselves).
