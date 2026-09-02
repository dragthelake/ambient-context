# Days and daily KB: design

Capture writes one folder per day holding three typed files instead of one
day file. An ingest step turns those files into a per-day knowledge base
with three agent calls, one per file. The daily summary reads the knowledge
base and the day's timeline, never a raw body.

## Why

Measured over eight days (25 August to 1 September 2026, 11.2 MB, 3,856
blocks), 37% of everything captured was feed content from x.com, YouTube,
Reddit and Hacker News that the summary prompt already tells the model to
ignore. Cross-block repetition was 73% of body text before day dedup. Metric
chrome was 22% of lines. The app captured its own window for 165 KB in one
day. On top of that volume, one agent call was asked to both structure the
evidence and interpret the day, and the two failures scored in five runs
came from that call misreading replayed material inside a file it could not
partition.

Three changes follow. Web pages become a visit row (URL, title, dwell) with
no body, because the URL is exact and the reader can open it. Blocks are
routed at capture time into files by kind, so each downstream prompt sees a
small input of one shape. And structuring is separated from interpreting:
three ingest calls produce a knowledge base, one summary call reads it.

## Layers

| Layer | Location | Written by | Mutable |
| --- | --- | --- | --- |
| Raw | `Days/YYYY-MM-DD/{apps,websites,messages}.md` | Capture, append-only | Never |
| KB | `KB/YYYY-MM-DD/*.md` | Ingest agent calls, split and validated by Rust | Regenerated on demand |
| Summary | `Summaries/YYYY-MM-DD.md` | Summary agent call | Regenerated on demand |
| Ledger | `Ledger/YYYY-MM-DD.md` | Rust, one entry per agent action | Append-only |

Deleting `KB/` or `Summaries/` is always safe; both rebuild from `Days/`.

## Folder layout

```
{capture folder}/
  Days/
    2026-09-02/
      apps.md
      websites.md
      messages.md
  KB/
    2026-09-02/
      manifest.md
      people.md
      commitments.md
      threads.md
      products.md
      issues.md
      reading.md
  Summaries/
    2026-09-02.md
  Ledger/
    2026-09-02.md
  AGENTS.md
```

Flat `YYYY-MM-DD.md` files at the root, written by 0.1.x, are ignored by
every reader. They are not migrated and not deleted.

## Raw files

All three files are created on first write with frontmatter:

```yaml
---
date: 2026-09-02
kind: apps | websites | messages
captured_by: Ambient Context 0.2.0
---
```

### `apps.md`

The day's timeline. Every finished block writes a heading here, in the
format the 0.1 day file used, so `days::parse_blocks` reads it unchanged:

```
## 09:14–09:41 · Zed · writer.rs

file: /Users/cameronsmith/Sites/ambient-context/src-tauri/src/writer.rs

<novel body lines>

## 09:41–09:48 · Arc · Tauri system tray

routed: websites

## 09:48–10:02 · Slack · #empty-build

routed: messages
```

A block of kind App keeps its `file:` line and body under the heading. A
block routed elsewhere gets a single `routed: websites` or
`routed: messages` line and nothing else.

### `websites.md`

A pipe table. The header row is written once with the frontmatter; one row
is appended per block of kind Website, in time order. The file is never
rewritten.

```
| start | end | app | domain | title | url |
| --- | --- | --- | --- | --- | --- |
| 09:41 | 09:48 | Arc | v2.tauri.app | Tauri system tray | https://v2.tauri.app/learn/system-tray/ |
```

`domain` is `rules::domain_of(url)`. A pipe inside a title or URL is
escaped as `\|`. A Website block whose URL never arrived has an empty
`domain` and `url` cell. Per-URL totals are computed at read time (see
`website_totals`), not stored.

### `messages.md`

Same block format as `apps.md`: heading, optional `url:` line, novel body
lines. Only blocks of kind Message appear here.

## Routing

A new module `route.rs` decides a finished block's kind from `(app, title,
url, rules)`. It runs at block close, inside `writer::append_block`, because
the URL of a browser block often arrives a few polls after the block opens
and the decision must see the final value.

```rust
pub enum Kind { App, Website, Message }
pub fn kind(rules: &Rules, app: &str, title: Option<&str>, url: Option<&str>) -> Kind
```

Precedence, first match wins:

1. A user rule with action `route_messages` matches the block. Rule
   matching and specificity are `rules::decide`'s, unchanged. An `exclude`
   rule still removes the block before it reaches the router.
2. The built-in message table matches. Apps, matched as case-insensitive
   substrings of the application name: Mail, Slack, Discord, Messages,
   Linear, Telegram, WhatsApp. URLs, matched as prefix on
   `domain_of(url)` plus path: `mail.google.com`, `outlook.live.com`,
   `outlook.office.com`, `github.com/*/*/pull/`, `github.com/notifications`,
   `linear.app/*/inbox`, `x.com/messages`, `x.com/notifications`,
   `reddit.com/message`, `linkedin.com/messaging`, `discord.com/channels`,
   `app.slack.com`. The table is a `const` in `rules.rs` beside
   `built_ins()` and is rendered in Settings as another always-on row.
3. `url` starts with `http://` or `https://`: Website. Any other scheme
   (`app://`, `file://`, `x-webdoc://`, `tauri://`, `about:`) or no URL:
   continue.
4. The application is a known browser (Safari, Chrome, Chromium, Arc,
   Firefox, Brave, Edge, Dia, Zen, Vivaldi, Opera) and there is no URL:
   Website with empty URL. A page whose URL was never exposed is still a
   visit, not a full-body app block.
5. Otherwise App.

`rules.json` gains `route_messages` as a fourth `Action` variant. The rules
editor in Settings offers it beside exclude, headings-only and full, with
the description "Record the body in messages.md".

## Writer and dedup

`writer::append_block(folder, block, dedup, shape, rules)`:

| Kind | `apps.md` | `websites.md` | `messages.md` | Dedup set |
| --- | --- | --- | --- | --- |
| App | heading, `file:`, novel body | | | novel lines added |
| Website | heading, `routed: websites` | one row | | untouched |
| Message | heading, `routed: messages` | | heading, `url:`, novel body | novel lines added |

Website block lines are discarded without entering the dedup set, for the
reason `headings_only` already has: a line first seen on a web page must
still be writable when it appears in an editor later.

`headings_only` applies on top of kind. A headings-only Message block
writes its heading to both `apps.md` and `messages.md` with no body. A
headings-only App block writes heading and `file:` only.

`DayDedup::roll_to` seeds one seen-set for the day from `apps.md` and
`messages.md` together, skipping frontmatter, `## ` headings, `file:`,
`url:` and `routed:` lines. The fresh-start check ("a deleted day file means
the user wants a fresh start") becomes: the day folder no longer exists.

`Shape` (`max_block_chars`, `write_references`) is unchanged and applies to
App and Message bodies.

## Prune and self-exclusion

Two changes to what is read:

- **Own window.** A built-in headings-only rule on the app's own process
  name (`ambient-context`, `Ambient Context`). The timeline records time
  spent in the app; the settings text and the summary pane are never
  recorded.
- **Own files.** `capture::is_own_output` returns true when the snapshot's
  `document` or `url` is under the capture folder (this already covers
  `Days/`, `KB/`, `Summaries/` and `Ledger/` because they are inside it),
  or when the window title contains a `YYYY-MM-DD` date together with the
  capture folder's name or one of `apps.md`, `websites.md`, `messages.md`,
  `manifest.md`. Editors that expose `AXDocument` (Zed, Obsidian, Writer,
  TextEdit) are caught by the path; the title check is the fallback.

Prune gains a per-kind pass at block close:

```rust
pub fn for_kind(kind: Kind, lines: Vec<String>) -> Vec<String>
```

Run on the finished block's lines before dedup. `normalise_line` at
snapshot time is unchanged. The first Message filters are chosen by
measuring the eight existing days for the highest-volume chrome in Mail,
Slack and Discord blocks (header rows, sidebar channel lists, member lists)
and encoding each as a regex with a fixture line from the real capture. App
and Website kinds ship with no extra filter. The measurement is a plan
task, not a design decision.

## Reading a day

`days.rs`:

- `list_days(folder)` scans `Days/*/` for directories named as dates.
  `DayEntry.bytes` is the sum of the three files. `days_in_month` and the
  defrag map follow unchanged.
- `read_day(folder, date, file: DayFile) -> Option<String>` with
  `DayFile::{Apps, Websites, Messages}`.
- `timeline(folder, date) -> String`: the `## ` headings of `apps.md`, one
  per line, nothing else.
- `website_totals(folder, date) -> Vec<UrlTotal>`: parses `websites.md`
  rows and merges by URL.

```rust
pub struct UrlTotal {
    pub url: String,
    pub domain: String,
    pub title: String,      // title of the longest visit
    pub dwell_secs: u64,
    pub visits: u32,
    pub first: String,      // HH:MM
    pub last: String,
}
```

Sorted by dwell descending. Rows with an empty URL merge by title instead.
`render_totals(&[UrlTotal]) -> String` produces the pipe table (domain,
title, dwell as `12m`, visits, first, last, url) that the Websites tab and
the `ingest_websites` prompt both use.

`parse_blocks` treats a `routed:` line like a `file:` line: recorded on
the block, never a body line.

## KB files

Every KB file carries frontmatter written by Rust:

```yaml
---
date: 2026-09-02
kind: kb
source: messages.md | apps.md | websites.md | none
generated_by: <ingest agent label>
prompt_sha256: <hash of the prompt that produced it>
---
```

The agent never writes frontmatter. Body conventions, enforced by
validation:

| File | From | Body |
| --- | --- | --- |
| `people.md` | messages | `## Name` per person; 1 to 5 lines: where (app, channel or thread), what was discussed, what was asked or agreed. |
| `commitments.md` | messages | `## I agreed to` and `## Owed to me`, each a task list: `- [ ] what · with whom · HH:MM-HH:MM · reference`. |
| `threads.md` | apps | `## Thread` per project or piece of work; 1 to 5 lines: what was done, files touched, what changed or was decided. |
| `products.md` | apps | `## Product` per tool, library or service used or evaluated; 1 to 3 lines on how it appeared. |
| `issues.md` | apps | `## Short title` per error, bug or blocker; symptom, where seen, whether it was resolved in the capture. |
| `reading.md` | websites | `## Topic` groups; one line per entry: title, domain, dwell. Feed browsing rolled up to one line per domain. |

Every line under a heading ends with a time citation `HH:MM-HH:MM` and,
where one exists, a `url:` or `file:` reference. A file with nothing to
report is exactly the line `Nothing evident.` with no heading. Every file is
present even when empty.

### `manifest.md`

Written by Rust after every call. Frontmatter only:

```yaml
---
date: 2026-09-02
calls:
  ingest_messages:
    disposition: accepted | rejected | failed | skipped
    input_sha256: <messages.md>
    timeline_sha256: <timeline>
    prompt_sha256: <prompt>
    engine: <agent label>
    at: 2026-09-03T06:00:12+10:00
  ingest_apps: ...
  ingest_websites: ...
---
```

`has_kb` is true when the manifest exists and at least one call is
`accepted`. `skipped` means the raw file did not exist and Rust wrote the
call's KB files as `Nothing evident.` with `source: none`.

## Ingest pipeline

Three agent calls, each with one input and a fixed set of outputs:

| Call | Input placeholder `{{INPUT}}` | Prompt | Writes |
| --- | --- | --- | --- |
| `ingest_messages` | `messages.md` whole | `ingest-messages.md` | `people.md`, `commitments.md` |
| `ingest_apps` | `apps.md` whole | `ingest-apps.md` | `threads.md`, `products.md`, `issues.md` |
| `ingest_websites` | `render_totals(website_totals)` | `ingest-websites.md` | `reading.md` |

Every prompt also receives `{{DATE}}` and `{{TIMELINE}}` so each call cites
times against the same clock. No two calls write the same file, so there is
no merge step and a failed call leaves the others' files valid.

### Prompts

Bundled under `src-tauri/prompts/`: `ingest-messages.md`,
`ingest-apps.md`, `ingest-websites.md`, `day-context.md`. Customised copies
live under `{config_dir}/prompts/` with the same names. `prompt.rs`
generalises to a `PromptId` enum with per-prompt `bundled()`, `path()`,
`current()`, `validate()`, `set()`, `reset()`. Validation per prompt:

- `day-context.md`: `REQUIRED_HEADINGS` (unchanged) and the placeholders
  `{{DATE}}`, `{{TIMELINE}}`, `{{KB}}`.
- Each ingest prompt: the placeholders `{{DATE}}`, `{{INPUT}}`,
  `{{TIMELINE}}` and the file markers for every file it writes.

### Output format

The agent returns one document. Each file is introduced by a marker line:

```
<<<file: people.md>>>
## Dan
...
<<<file: commitments.md>>>
## I agreed to
...
```

Text before the first marker is ignored. A `## Reasoning` section after the
last file, introduced by `<<<reasoning>>>`, is recorded in the ledger's
`reasoning` field and not written to the KB.

### Input cap

`ingest_max_chars` (setting, default 400,000) caps `{{INPUT}}`. When the
input is over the cap, block bodies are trimmed longest-first, headings and
reference lines kept, each trimmed block ending in `[trimmed n lines]`,
until under the cap. The number of trimmed blocks is written into the
ledger entry's `reasoning` prefix as `input trimmed: n blocks`.

### Validation

In Rust, per call, before any file is written:

1. Every expected file marker is present exactly once.
2. Each file is either exactly `Nothing evident.` or every non-heading,
   non-blank line carries at least one citation matching
   `\b\d{2}:\d{2}-\d{2}:\d{2}\b` whose start and end both fall inside some
   block of that day's timeline (inclusive of boundaries).
3. Each file is at most `MAX_KB_LINES` (200) lines.
4. Unfenced: `summarise::unfence` is applied first so a fenced reply is not
   rejected for its fence.

Failure writes the whole output to
`{app_data}/rejected-ingest/{date}-{call}.md`, records a `Rejected` ledger
entry with the reason, updates the manifest with `disposition: rejected`,
and leaves the call's previous KB files (if any) in place.

### Writes

Atomic per call: files are written to `KB/.tmp-{date}-{call}/`, then each
is renamed into `KB/{date}/`, then the manifest is rewritten. A crash
between renames leaves at worst one call's files half-updated; the next
`needs_ingest` sees a hash mismatch and re-runs that call.

### `needs_ingest(folder, date, call) -> bool`

True when any of: no manifest; the call is absent from the manifest; the
call's disposition is not `accepted` or `skipped`; the recorded
`input_sha256`, `timeline_sha256` or `prompt_sha256` differs from the
current values. Re-ingest from the UI or MCP ignores this and runs all
three.

### Ledger

One entry per call. `action` is `ingest_messages`, `ingest_apps` or
`ingest_websites`. `prompt_id` is the prompt file stem. `inputs` lists the
raw file and the timeline (as a virtual path `Days/{date}/timeline`) with
their hashes. `engine` is the ingest agent's label. `output` is the whole
agent document. `reasoning` is the `<<<reasoning>>>` section.

## Summary from the KB

`day-context.md` takes `{{DATE}}`, `{{TIMELINE}}` and `{{KB}}`. `{{KB}}` is
the six files concatenated, each under a `# people.md` style header, with
frontmatter stripped. The prompt's "How to read the input" section
describes the three inputs: the KB is the evidence and every claim in it
already carries a citation; the timeline is the clock; raw bodies were
omitted on purpose; `Nothing evident.` means the ingest found nothing, not
that capture was missing. Output headings, `REQUIRED_HEADINGS`,
`MAX_SUMMARY_LINES` and `summarise::validate` are unchanged.

`summarise_day` ledger `inputs` lists the six KB files and `apps.md` with
their hashes.

### Pipeline

```
run_day_pipeline(date, trigger, force_ingest):
  for call in [messages, apps, websites]:
      if force_ingest || needs_ingest(date, call):
          ingest(call)            // ledger: ingest_*; Err stops here
  summarise_day(date)             // ledger: summarise_day
```

Header **Summarise**, MCP `summarise_day` and the scheduled job run the
pipeline with `force_ingest = false`. **Ingest** runs the ingest loop only,
gated by `needs_ingest`, and skips the summary. **Re-ingest** runs the
ingest loop only with `force_ingest = true`. The
scheduled batch treats a pipeline failure on one day the way it treats a
summary failure today: that day fails, the batch stops, the next tick
retries.

Model calls per summarised day: three ingest, one summary. A day with no
`messages.md` makes two ingest calls; `ingest_messages` is skipped.

## Settings

New fields on `Settings`, all `serde(default)`:

| Field | Type | Default | Purpose |
| --- | --- | --- | --- |
| `ingest_agent` | `Option<Agent>` | `None` | Runs the three ingest calls. `None` means use `agent`. |
| `ingest_max_chars` | `usize` | `400_000` | Cap on `{{INPUT}}` per call. |

No enable flag for ingest: summarising always runs the pipeline.

## UI

### Day view

Three modes: **Raw**, **KB**, **Summary**.

- Raw has a segmented control: Apps, Websites, Messages. Apps and Messages
  render through the existing block list. Websites renders `website_totals`
  as a table (domain, title, dwell, visits) with the URL revealed on hover
  and opened on click.
- KB has a segmented control over the six files, rendered as markdown, and
  an info button that shows the manifest. Empty state per file:
  "Nothing evident." or "Not ingested yet".
- Summary is unchanged.

Header actions: **Summarise**, **Ingest**, **Re-ingest**, **Open folder**
(reveals `Days/{date}/`). Job status text names the step: `ingesting
messages (1 of 3)`, `ingesting apps (2 of 3)`, `ingesting websites (3 of
3)`, `summarising`.

`open_in_editor(date, which)` accepts `apps`, `websites`, `messages`, `kb`
(opens `KB/{date}/threads.md`) and `summary`.

### `DayEntry`

Gains `has_kb: bool`. Defrag map colours are unchanged; KB presence is
shown in the Day header, not as a fourth colour.

### Agent tab

- A second agent picker labelled **Ingest** with a **Same as summary**
  option selected by default.
- A prompt selector (Summary, Ingest messages, Ingest apps, Ingest
  websites) above the existing single prompt editor. Customised, Reset and
  validation messages apply to the selected prompt.
- **Longest ingest input** (`ingest_max_chars`) beside the timeout.

### Settings, rules

The rules list shows the built-in message table as an always-on row and
offers `route_messages` as an action on user rules.

## MCP

| Tool | Change |
| --- | --- |
| `read_day` | Gains optional `file: "apps" \| "websites" \| "messages"`, default `apps`. |
| `read_kb` | New. `date`, optional `file` (one of the six or `manifest`); no `file` returns all six concatenated. Reads the capture folder directly. |
| `ingest_day` | New. `date`, optional `force`. Queues the three ingest calls via the control socket, returns a job id. |
| `summarise_day` | Docs updated: runs the pipeline. |
| `list_days` | Unchanged in shape; `has_kb` added to each entry. |

`docs/mcp.md` and `tests/docs_match_tools.rs` updated together.

## `AGENTS.md`

Rewritten to describe `Days/`, `KB/`, `Summaries/` and `Ledger/`, the three
raw file formats, the six KB files and their trust level (derived,
regenerable, every line cited), and the reading order: summary for what a
day meant, KB for the structured evidence, `Days/` for the record.
`writer::ensure_agents_file` replaces the previous bundled copy by hash as
today.

## Decisions

| Decision | Choice | Reason |
| --- | --- | --- |
| Web page bodies | Not captured | The URL is exact and openable; bodies were 37% feed noise. |
| Route decision point | Block close, in the writer | The URL arrives late; a per-snapshot decision would split every page visit into two blocks. |
| Message classification | Built-in table plus `route_messages` user rules | Works on a fresh install, overridable without a release. |
| Social sites | Inbox and notification URLs only | Interaction is not visible in the accessibility tree; URL is the only reliable signal. |
| Meta description | Not captured | Would need HTTP fetches from a local-only recorder. |
| `websites.md` shape | Append-only log, totals at read time | Keeps every raw file append-only. |
| Raw immutability | Typed files are the record; prune is capture-time only | No second copy, no rewrite, audit trail intact. |
| 0.1 day files | Ignored, not migrated | One user, eight days, different prune rules. |
| KB scope | Per day | No merge policy; regenerable from `Days/` alone. |
| Ingest output | One stdout document, split in Rust | Works with any agent CLI; validated before any write; ledger keeps the whole output. |
| Ingest calls | One per raw file, partitioned outputs | Small inputs, no merge, partial failure leaves valid files. |
| Ingest agent | Optional separate agent | Cheap model on structure, strong model on interpretation. |
| Summary inputs | Timeline plus KB only | The summariser interprets cited evidence and never sees a raw body. |
| `generated_by` | Written by Rust from the agent label | Self-reported model names were inconsistent. |

## Out of scope

- Persistent cross-day wiki or entity merge.
- Replay marker (`[replay: DATE]`) and its prompt rule.
- Citation and token validators on the summary output.
- Migrating 0.1 day files.
- Changing the segmenter, `max_block_chars` default, or `normalise_line`.
- Meta description or any HTTP fetch from the recorder.

## Testing

- `route.rs`: table-driven test over `(app, title, url)` for every
  precedence step, every built-in table entry, `exclude` beating
  `route_messages`, and each browser name without a URL.
- `writer.rs`: three blocks (Zed with file, Arc with URL, Slack) into a
  temp folder; assert exact content of all three files; Website lines
  absent from dedup; restart re-seeds from both `apps.md` and
  `messages.md`; headings-only Message block; pipe escaping in titles.
- `prune.rs`: each Message filter against its fixture line; `for_kind` is
  identity for App and Website.
- `capture.rs`: `is_own_output` for a `KB/` document path and for a title
  carrying a date plus `messages.md`.
- `days.rs`: `list_days` ignores flat 0.1 files; `website_totals` merges
  three visits to one URL and ranks by dwell; empty-URL rows merge by
  title; `timeline` is headings only; `parse_blocks` keeps `routed:` out of
  the body.
- `ingest.rs`: split a fixture document into files; reject a missing
  marker, a line without a citation, a citation outside the timeline, and a
  file over `MAX_KB_LINES`; accept `Nothing evident.`; atomic write leaves
  no `.tmp` folder; `needs_ingest` on each manifest state; input trimming
  keeps headings and trims longest-first.
- `prompt.rs`: each prompt's validation rejects a missing placeholder or
  marker; bundled prompts all validate.
- `jobs.rs`: pipeline records three `ingest_*` entries then
  `summarise_day`; a rejected `ingest_apps` stops before summary and leaves
  `people.md`; a missing `messages.md` records `skipped`; `force_ingest`
  re-runs an accepted call.
- Frontend: Raw tabs, Websites table, KB tabs and empty states, Ingest and
  Re-ingest actions with step text, ingest agent picker default, prompt
  selector switching the editor.
- `docs_match_tools.rs` passes with `read_kb` and `ingest_day`.
- Manual QA: capture a mixed hour (Zed, Arc on docs, x.com home, x.com
  messages, Mail, Slack); confirm each block landed in the right file and
  `websites.md` has no bodies; Ingest; inspect all six KB files for
  citations; Summarise; confirm the ledger shows four entries with the
  expected inputs; delete `KB/{date}/`, Re-ingest, confirm regeneration.

## Version

Ships as 0.2.0.
