# Review: Days and daily KB (0.2.0)

Range reviewed: `9197b8f..e6ec2d2`, 14 commits, 58 files, ~8,300 insertions.

Spec: `docs/superpowers/specs/2026-09-02-days-and-kb-design.md`
Plan: `docs/superpowers/plans/2026-09-02-days-and-kb.md`

Reviewed in four passes: Rust capture side (`route.rs`, `writer.rs`,
`prune.rs`, `capture.rs`, `redact.rs`, `rules.rs`, `days.rs`,
`summarise.rs`), Rust ingest and jobs (`ingest.rs`, `jobs.rs`, `prompt.rs`,
`settings.rs`, the four bundled prompts), frontend (`DayView`, `DayHeader`,
`RawPane`, `WebsitesPane`, `KbPane`, `AgentTab`, `PromptSettings`, CSS,
tests), and MCP plus docs (`mcp/tools.rs`, `mcp/files.rs`, `mcp/client.rs`,
`control.rs`, `ipc.rs`, `lib.rs`, `docs/mcp.md`, `AGENTS.md`, `CHANGELOG`,
handover). `route.rs`, `writer.rs`, `days.rs`, `ingest.rs`, `jobs.rs` and
`prompt.rs` were read in full rather than as hunks.

Verified locally: `cargo test` passes (396 unit, 5 integration at current
HEAD), `npx vitest run` passes (73). Nothing in the working tree was
mutated; this file is the only write.

**Note on HEAD.** The checkout sits two commits past the reviewed head:
`5c475e3` (handover wording) and `f6f64eb` (escape heading-like body lines).
Two findings below were introduced inside the reviewed range and fixed by
`f6f64eb`; they are recorded as such rather than dropped, because the range
is what was asked for.

---

## Strengths

- **The stub agents are real subprocesses, not mocks.** `jobs.rs:931-947`
  builds an `Agent` pointing at `/bin/sh`, and
  `the_pipeline_runs_three_calls_then_the_summary_and_skips_what_is_current`
  (`jobs.rs:1094`) drives a shell `case` over stdin so one script answers
  differently per call. The pipeline tests therefore exercise
  `agent::run_with_env`, process spawn, stdin write, stdout capture and the
  timeout path end to end. This is the most valuable thing in the diff's
  test suite and it was not something the plan spelled out.
- **Cross-midnight is handled consistently in all three places it matters.**
  `days::spans` (`days.rs:99-101`) carries an end before its start past
  1440; `website_totals` (`days.rs:164`) does the same for dwell; and
  `ingest::inside` (`ingest.rs:185-190`) accepts a citation minute either
  directly or shifted by 1440, so a `23:50-00:10` block validates at both
  endpoints. All three are covered by tests.
- **Pipe escaping round-trips correctly, including the awkward case.**
  `row_cells` (`days.rs:124-145`) carries `\|` through as two characters and
  lets `unescape_cell` reverse it, so a title that genuinely contained a
  backslash before a pipe (`a\|b`, written `a\\|b`, read back `a\|b`)
  survives intact rather than losing a character. The empty-URL
  merge-by-title path is tested against an escaped title.
- **Two deviations from the plan are both corrections of plan bugs**
  (detailed below): the directory-creation ordering in `append_block`, and
  the validation ordering in `ingest::validate`. Copying the plan verbatim
  would have failed the plan's own tests in both cases.
- **The dedup contract holds exactly as specified.** Website block lines
  never reach `novel_lines` (`writer.rs:330-339`); message lines are pruned
  first and the *pruned* form is what enters the seen-set
  (`writer.rs:341-348`); `roll_to` seeds from `apps.md` and `messages.md`
  together and skips frontmatter, headings, `file:`, `url:` and `routed:`.
  Each is covered by a test that would fail if the order were reversed.
- **No Unicode byte-slicing hazards were introduced.** Every `&s[..n]` added
  in the diff (`ingest.rs:500-501`, `route.rs:61`) indexes off a `find`
  result or a known-ASCII prefix. `Invalid::NoCitation` truncates with
  `chars().take(80)` rather than a byte slice, which is exactly the mistake
  `mcp/files.rs` `heading_time` once had.
- **Atomicity and crash recovery are right.** `write_call` renames into
  place and `record_call` writes the manifest last, so a crash between the
  two leaves stale hashes and `needs_ingest` re-runs the call. That is what
  the spec asked for and the ordering is easy to get backwards.
- **Writing standards are clean.** Zero em-dashes (U+2014) across all 58
  changed files. No American spellings in prose (the only `normalize`,
  `Serialize` and `overscroll-behavior` hits are code identifiers). No slop
  phrasing. Comments explain why, not what, throughout.
- **`needs_ingest` covers the prompt hash correctly.** `run_day_pipeline`
  (`jobs.rs:310`) hashes `p.prompts.for_call(call)`, which is the current
  prompt (the customised copy if one exists, the bundled one otherwise), so
  editing a prompt in Settings re-runs that call and only that call.

---

## Issues

### Critical (Must Fix)

None. Nothing in the range corrupts the append-only record, breaks a gate,
or leaves the app unable to start.

### Important (Should Fix)

**1. `src-tauri/src/route.rs:105-107`: `is_browser` matches on a
case-insensitive substring, which silently discards native app bodies.**

```rust
pub fn is_browser(app: &str) -> bool {
    BROWSERS.iter().any(|b| contains_ci(app, b))
}
```

`BROWSERS` contains `"Dia"`, `"Zen"` and `"Arc"`. `contains_ci` means any
app whose name contains those letters anywhere matches: `"dia"` is a
substring of `"Media"`, so anything named "Elmedia Player", "Media Encoder"
or similar is classified as a browser. A browser block with no URL becomes
`Kind::Website`, which writes a heading plus `routed: websites` to `apps.md`
and a visit row with an empty URL to `websites.md`. The block's body is
discarded and never enters the dedup set, so it is unrecoverable. For a
recorder whose entire value is the record, that is silent data loss rather
than misfiling.

The same substring rule on `MESSAGE_APPS` (`route.rs:24-26`, checked at
`route.rs:115`) is less harmful because the body is still written, but
`"Linear"` matches "Linearity Curve" (a real design app) and `"Zen"` matches
"Zendesk", so those bodies land in `messages.md` and then get the Mail-chrome
prune applied to them.

Fix: match the whole application name case-insensitively, or require the
match to sit on a word boundary. The MCP and Settings surfaces already show
the table to the user, so an exact-match list is no less discoverable.

**2. `src/main-window.css:527-535`: `.websites-pane` is missing the frame
every sibling pane has.**

```css
.websites-pane {
  display: flex;
  flex-direction: column;
  min-height: 0;
}
```

`.raw-pane` (line 505) and `.kb-pane` (line 417) both carry `flex: 1;
padding: 2px; overflow: hidden; background: var(--well); box-shadow:
var(--bevel-in); margin: 10px 12px 12px;`. `.websites-pane` has none of
them. Switching Raw from Apps to Websites will drop the inset well, lose the
12px margin, sit flush against the header, and (without `flex: 1`) not fill
the available height, so the table will not scroll inside a frame the way
the other two panes do. This came straight from the plan's CSS snippet,
which was written without reconciling against `.raw-pane`.

Fix: give `.websites-pane` the same six declarations as `.raw-pane`, and
`.websites-pane-scroll` the same treatment as `.raw-pane`'s scroller.

I have not run the app, so this is read from the stylesheet rather than
seen. It is the first thing to look at in manual QA.

**3. `src-tauri/assets/AGENTS.md:41-42`: the `websites.md` columns are
documented wrong.**

> `websites.md` is a pipe table of visits: domain, URL, title, dwell and
> visit counts, first and last seen.

The file the writer actually produces has the header
`| start | end | app | domain | title | url |` (`writer.rs:173`, with
`render_website_row` at `writer.rs:201-213`). There is no dwell column, no
visit count and no "first and last seen". Those are the computed
`render_totals` columns (`days.rs:204`) that exist only in memory and in the
ingest prompt's input. AGENTS.md is the contract with the reading LLM, and
it is telling that LLM to look for four columns that are not there.

Fix: describe the six real columns, and say that per-URL totals are derived
at read time rather than stored.

**4. `src-tauri/assets/AGENTS.md:38-39`: `routed:` is documented as
messages-only.**

> A `routed:` line on a block in `apps.md` means the body was written to
> `messages.md` instead.

`routed: websites` is written just as often (`writer.rs:280-283`,
`Kind::routed_name`). A reading LLM that encounters it has been told nothing
about what it means. The bundled `ingest-apps.md` prompt gets this right
("A block whose body is only `routed: websites` or `routed: messages`..."),
so the two documents disagree.

Fix: name both values and where each body went.

**5. `src-tauri/src/days.rs:78` (at `e6ec2d2`): a captured body line
beginning `## ` forged a timeline block. Fixed at `f6f64eb`.**

At the reviewed head, `timeline()` filtered on `l.starts_with("## ")`, and
`prune::normalise_line` did not escape heading-like lines. A block body
containing `## 09:14-09:41 · Zed · x`, which happens whenever the user looks
at a day file, a KB `commitments.md` with its `## Owed to me` heading, a
diff, or a plan quoting one, was emitted into the timeline as a real
heading. That has two consequences beyond a wrong-looking timeline:
`spans()` gains a phantom range, so `ingest::validate` accepts citations to
time the user never spent there; and `parse_blocks` splits one block into
two in the Raw view.

`f6f64eb` fixes both ends (escape at capture, and `timeline()` now requires
`parse_heading` to succeed). Recorded because it was live in the reviewed
range, and because one residual hole remains: see Minor 3.

**6. `src-tauri/src/ingest.rs:472-486`: `needs_ingest` trusts the manifest
and never checks that the KB files exist, and `summarise_day` never refuses
an empty KB.**

`needs_ingest` compares three hashes against `manifest.md`. If the KB `.md`
files are deleted but `manifest.md` survives (a partial cleanup, a sync
conflict, a user tidying), every call reports "accepted, unchanged", every
call is skipped, and `summarise_day` (`jobs.rs:235`) then builds `{{KB}}`
from `kb_for_prompt`, which returns `(not ingested)` for all six files. The
summary runs anyway, off the timeline headings alone, and nothing in the
ledger or the UI says the evidence was missing.

The happy path is safe: `run_day_pipeline` always runs ingest first, an
`ingest_apps` failure stops before the summary, and deleting the whole
`KB/{date}/` folder takes the manifest with it. It is specifically the
manifest-survives-its-files case that goes quiet.

Fix, cheapest first: have `needs_ingest` also require that every file in
`call.files()` exists in `kb_dir`. Optionally, have `summarise_day` return
an error when all six read as `(not ingested)`, since the pipeline has no
legitimate path to that state.

### Minor (Nice to Have)

**1. `src-tauri/src/jobs.rs:222-225`: a failed KB write loses the agent
output.** `write_call` failing returns `Err` before `record_in_ledger`, so
the whole ingest document (already sitting in `entry.output`) is dropped and
there is no ledger entry at all. The failed and rejected paths both ledger
before returning; this one should too.

**2. `src-tauri/src/days.rs:212` against
`src/components/WebsitesPane.tsx:5-7`: dwell is truncated in Rust and
rounded in TypeScript.** `render_totals` writes `t.dwell_secs / 60`;
`minutes()` returns `Math.round(secs / 60)`. A 90-second visit reads `1m` in
the ingest prompt and `2m` in the Websites tab. Pick one.

**3. `src-tauri/src/prune.rs:120-135`: `clean_message_line` can unescape a
forged heading.** It strips `U+FFFC` and `U+00AD`, neither of which is in
`ZERO_WIDTH` (`prune.rs:7`), so `normalise_line` never saw them and never
escaped the line. A captured line `\u{fffc}## Owed to me` therefore reaches
`messages.md` as an unescaped `## Owed to me`. `messages.md` is not the
timeline source so no phantom span results, but `parse_blocks` and the MCP
`read_day` time filter both treat it as a block boundary. Simplest fix: run
`escape_heading` at the end of `clean_message_line` too.

**4. `src-tauri/src/ingest.rs:448-459`: manifest values are unquoted YAML.**
`write_manifest` interpolates `engine` and the hashes raw into what is
declared as frontmatter. An agent label containing `": "` would break the
line-based round trip in `read_manifest`, and any external YAML parser
reading `manifest.md` gets nothing useful from a value with a colon. Quoting
the values costs one `format!` change.

**5. `src-tauri/src/mcp/files.rs:127-129`: `NoKb` is the wrong message for a
missing file.** `read_kb(folder, date, Some("people.md"))` on a day whose KB
exists but has no `people.md` returns `None` and surfaces "There is no
knowledge base for {date} yet. Call ingest_day to build one." Distinguish
the two cases.

**6. `src-tauri/src/ingest.rs:182`: the citation regex requires zero-padded
hours.** `\b(\d{2}):(\d{2})[-–](\d{2}):(\d{2})\b` rejects `9:48-9:59`. Block
headings are always zero-padded so a well-behaved model will comply, but the
consequence of a stray single-digit hour is that the whole call is rejected
and re-run. Accepting `\d{1,2}` costs nothing.

**7. `src-tauri/src/mcp/files.rs:87`: mixed `&&` and `||` without
parentheses.** `if from.is_none() && to.is_none() || file ==
DayFile::Websites` parses as the intended `(a && b) || c`, but the plan
wrote it with parentheses and the line reads ambiguously.

**8. `src-tauri/src/ingest.rs:294-306`: `trim_input`'s loop is O(n) per
iteration.** `total()` re-counts characters across every line of the whole
input on each trim. At the 400,000-character default with a few hundred
trims that is tens of millions of char counts. Keeping a running total and
subtracting the trimmed block's length would make it linear. Not urgent,
since trimming only fires on an over-cap day, but it is the one place in the
diff that scales badly.

**9. `src/components/KbPane.tsx:25-26`: Rust doc-comment syntax in
TypeScript.** `/// Headings, task lines, bullets and paragraphs;` should be
`//` or a JSDoc block. Copied from the plan. The same pattern is in
`src/lib/days.ts:44`, which predates this change.

**10. `src/components/DayView.tsx:290-303`: an ingest failure is reported as
a summary failure.** The `catch` and the `job_state` `failed` branch both
write `manualFailure`, which feeds `SummaryState` (`DayView.tsx:373`), so a
failed `ingest_apps` shows in the Day header as a failed summary. The step
text covers the running case; the failure case does not distinguish.

**11. `src/components/DayView.tsx:280`: `setMode("kb")` runs before
`ingest_now` is known to have succeeded.** Pressing Ingest with no agent
connected switches to an empty KB pane and then shows the error.

**12. `src/components/DayHeader.tsx:59`: Open in KB mode always opens
`threads.md`.** `target_path(.., "kb")` (`lib.rs:206`) is hard-coded to
`threads.md` per the spec, but the KbPane defaults to `people.md`, so Open
never opens the tab in view.

**13. `src/components/PromptSettings.tsx:26-35`: switching prompt discards
an unsaved draft.** `read` re-runs on `id` change and overwrites `draft`
with no warning. Four prompts in one editor makes this easier to hit than it
was with one.

**14. `src-tauri/src/control.rs:410-455` and `:542`: three near-identical
rejection arms.** `set_prompt` now has `Empty`, `MissingHeading` and a
catch-all `Err(error)`, each repeating the same `ledger_write` plus
`Response::err` body with a different `reason` string. Collapsible to one
arm computing `reason` from the error.

**15. `src-tauri/src/capture.rs:70-86`: a summary opened in a title-only
editor is no longer self-excluded.** `OWN_FILES` is `apps.md`,
`websites.md`, `messages.md`, `manifest.md`. A window titled `2026-09-02.md`
(which is what `Summaries/2026-09-02.md` looks like in an editor that
exposes no `AXDocument`) has a date but matches neither `OWN_FILES` nor the
folder name, where 0.1 caught it via `{today}.md`. Spec-conformant, since
the spec named exactly those four files, but a small regression worth
knowing about.

**16. `src-tauri/src/mcp/tools.rs:349-353`: a non-string `file` argument
silently defaults to `apps`.** `arguments["file"].as_str()` returns `None`
for a number or an object, which `.unwrap_or(Some(Apps))` turns into the
default rather than the "file must be one of..." error a string typo gets.

**17. Spec-listed tests not written.** The spec's Testing section names four
cases that are absent or only partly covered: `exclude` beating
`route_messages` in `route.rs` (moot in practice, since `redact_snapshot`
drops an excluded snapshot before the router, but the spec asked for it); "a
rejected `ingest_apps` stops before summary and leaves `people.md`" is
tested at the `ingest_call` level only, never through `run_day_pipeline`;
`needs_ingest` is untested for the `rejected` and `failed` dispositions
(both fall to the `_ => true` arm); and `KbPane` refresh on job done is
untested from `DayView`, because that test's `job_state` mock returns
`"running"` forever.

---

## Deviations from the plan

**1. `writer.rs:315-358`: `fs::create_dir_all(day_dir(..))` moved from
before the routing decision to inside each match arm, after
`dedup.novel_lines`. Improvement, and a necessary one.** The plan (Task 2
Step 5) created the directory as the first statement of `append_block`. The
fresh-start check in `novel_lines` (`writer.rs:80`) is
`!self.seen.is_empty() && !day_dir(folder, date).exists()`. With the plan's
ordering the directory would always exist by the time that ran, so deleting
a day folder mid-session would never clear the seen-set and the rewritten
day would come back as bare headings. The plan's own test
`a_deleted_day_folder_means_a_fresh_start` would have failed.

**2. `ingest.rs:196-208`: `validate` checks `MissingFile` before
`UnexpectedFile`. Improvement.** The plan checked unexpected files first,
which would have made its own test `a_missing_file_is_rejected` (rename
`commitments.md` to `promises.md`, expect `MissingFile("commitments.md")`)
return `UnexpectedFile("promises.md")` instead. The implemented order also
gives the better error: telling the user which file is missing is more
actionable than telling them which one is surplus.

**3. `jobs.rs:300` and `jobs.rs:339`: `on_step: impl FnMut(&str)` instead of
`&mut dyn FnMut(&str)`. Improvement.** Static dispatch, no
double-indirection, and the call sites read better (`|_| {}` rather than
`&mut |_| {}`). No behavioural difference.

**4. `days.rs:78`: `timeline()` filters on `parse_heading(l).is_some()`
instead of `l.starts_with("## ")`. Improvement, but landed one commit past
the reviewed head** (`f6f64eb`). See Important 5.

**5. `prune.rs:113-127`: `escape_heading` added to `normalise_line`. An
unplanned improvement**, also at `f6f64eb`. Nothing in the spec or plan
anticipated captured text forging a block boundary. Finding and closing it
is good work; the residual `U+FFFC` path (Minor 3) is worth finishing.

**6. `control.rs:542`: a catch-all `Err(error)` arm added to
`writes::set_prompt`. Neutral, tending to a problem.** Necessary once
`PromptError` gained `MissingPlaceholder`, `MissingMarker` and `Io`, but it
was added beside two existing arms with identical bodies rather than
replacing them (Minor 14). It also means a new `PromptError` variant will
silently fall through with its `Display` text instead of getting the
considered MCP-facing wording the other two got.

**7. `agent.rs:359`: `#[allow(dead_code)]` on `claude_code_args`. Neutral.**
The function became unused when the prompt work changed its caller, and the
attribute papers over that rather than deleting it. The comment explains
why, so it is a deliberate choice rather than an oversight.

Everything else in the plan is present as written: the routing precedence
and every built-in table entry, the `route_messages` action and its two
built-in Settings rows, the per-kind prune filters, `list_captured` ignoring
flat 0.1 files, `parse_blocks` keeping `routed:` out of the body, the
split, validate, trim, atomic-write and manifest semantics including
`skipped`, the step text, the four `PromptId`s and their per-id validation,
`read_kb`, `ingest_day`, `read_day file`, and the twenty-tool docs test.

---

## Recommendations

1. Fix Important 1 (`is_browser` substring) before manual QA, because it
   determines what gets recorded, and every hour of QA capture taken against
   the current rule may misfile a native app.
2. Fix Important 2 (`.websites-pane` CSS) before opening the app, then look
   at all three Raw tabs and the KB tab side by side at a real window size.
   The KB segmented control has seven segments in a pane with
   `overflow: hidden`; check it does not clip.
3. Fix Important 3 and 4 (AGENTS.md) in the same pass. They are two
   sentences, and AGENTS.md is the only thing standing between a reading LLM
   and a misread `websites.md`.
4. Run the spec's manual QA in full. Nothing in the automated suite touches
   the accessibility reader, the real routing of a live Slack or Mail
   window, the prune filters against real mail chrome, or what any of the
   three new panes actually look like. The handover already says this; it is
   the single largest remaining risk in the release.
5. During QA, watch the ingest cost specifically. `needs_ingest` compares
   the timeline hash, so any new block on a still-recording day invalidates
   all three calls: pressing Ingest twice on today costs six agent calls,
   not one. That is the spec's design, since citations are validated against
   the timeline, and it is bounded for finished days, but confirm the
   behaviour matches the expectation before the scheduled job runs
   unattended.
6. Consider having `needs_ingest` also assert that the KB files exist
   (Important 6). It is three lines, and it closes the only path found where
   a summary is built from no evidence without saying so.

---

## Assessment

**Ready to merge?** With fixes

**Reasoning:** The architecture matches the spec closely, the two departures
from the plan are both corrections of plan bugs, and the pipeline tests
exercise a real agent subprocess rather than a mock, which is unusually
strong for this shape of work. The blocking items are small and specific:
`is_browser`'s substring match silently discards native app bodies,
`.websites-pane` is missing the frame its sibling panes have, and AGENTS.md
misdescribes the `websites.md` columns and the `routed:` line to the LLM it
exists to inform. None of that is structural, but manual QA has not been run
at all, so no claim can be made about how any of this looks or behaves in
the live app.
