# Privacy and security

Ambient Context processes an unusually sensitive source: text visible in
the window you are actively viewing. Its posture is local-first and
data-minimising, but it is not a data-loss-prevention product. Redaction
rules are defence in depth, not a guarantee that every secret is removed.

This document describes **1.0.0**. It separates structural properties from
tested controls, best-effort heuristics, and choices that sit with you.

## Trust boundary

Ambient Context trusts:

- the local macOS account and filesystem permissions
- macOS Accessibility and frontmost-window reporting
- applications to expose an accurate Accessibility tree and to mark secure
  fields correctly
- you to choose an appropriate capture folder
- any agent CLI or MCP client you point at that folder

It does not encrypt day files, manage retention, control backups, or
constrain what a separate LLM client does with the files. Another process
or person that can read your files can read the record.

## Data inventory

| Data | Lifetime | Location | Notes |
| --- | --- | --- | --- |
| Focused-window snapshot | One poll | Process memory | Bounded Accessibility walk; secure subtrees skipped at source |
| Open dwell block | Until the window changes, capture stops, or idle close | Process memory | Redacted and pruned text only |
| Day-level dedup set | Current day / folder | Process memory | Non-cryptographic hashes of admitted lines |
| Day files | Until you delete them | `Days/YYYY-MM-DD/{apps,websites,messages}.md` | Plaintext Markdown |
| Knowledge base | Regenerable | `KB/YYYY-MM-DD/` | Derived from day files by an agent you configure |
| Summaries | Regenerable | `Summaries/YYYY-MM-DD.md` | Derived; may paraphrase sensitive content that made it into the day files |
| Ledger | Until you delete it | `Ledger/YYYY-MM-DD.md` | Model actions, prompt ids, input hashes, dispositions |
| Settings | Until changed | Tauri app config directory | Plaintext JSON (folder path, agents, rules, schedule) |
| Rejected agent output | Until you delete it | App support `rejected/` | Kept for debugging failed runs; can contain day content |

There are no screenshots, video, audio, embeddings, accounts or telemetry
events in this inventory.

## What the capture path does not do

- It does not take screenshots, record the screen, capture audio, or run
  OCR. The only content reader is the macOS Accessibility bridge.
- It does not enumerate background windows or other displays. Each poll
  asks for the focused window of the frontmost application.
- It returns no snapshot while macOS reports the session locked
  (`ERROR: screen locked` from the Swift bridge).
- It does not upload day files, KB files or summaries to an Ambient Context
  service. There is no such service.

## Control layers

### Before text is read

The Swift Accessibility walker skips `AXSecureTextField` roles and secure
subroles, and does not traverse those subtrees. Strength depends on the
target app marking sensitive controls correctly.

### Before a snapshot is kept

Rust drops whole snapshots for:

- recognised password-manager application names (substring match)
- window titles containing private-browsing markers (`private browsing`,
  `incognito`, `inprivate`)

The Ambient Context window itself is forced to headings only, so settings
and summaries are not re-ingested as body text. Opening the day's own
files in an editor is treated as own output and is not recorded as a
normal block.

### Before lines are written

Built-in regex patterns replace recognised credential shapes, API-style
keys, bearer tokens, labelled `password=` / `api_key=` forms, and
card-shaped digit runs with `[redacted]`. You can add extra patterns in
Settings. Patterns that fail to compile are ignored so a typo cannot stop
capture.

Website visits are recorded as URL rows without page bodies. Message
surfaces are routed to `messages.md` with chrome pruned; that is for
readability, not secrecy.

### After writing

Day files are append-only plaintext. KB and summaries are regenerable
derivatives. Anyone or any tool with folder access can read them. Prefer a
folder outside iCloud Drive (and similar sync roots) if you want the files
to stay on this computer only; Setup warns when the chosen path looks
synced.

## Network and side channels

| Channel | When | What leaves |
| --- | --- | --- |
| GitHub Releases updater | When you check for updates | Version check against the configured `latest.json` endpoint; download if you install |
| `open_link` | When you press a link in the UI | Opens a URL in your default browser |
| Agent CLI | When you process a day | Prompt and capture excerpts you already stored, under that CLI's own network and retention policy |
| MCP client | When a client connects | Whatever that client reads or writes over the local socket / stdio bridge |

Capture itself does not open network connections to move day content.

## Downstream agents

Ingest and summarise run through an agent command you configure (for
example Claude Code or Codex). Ambient Context does not host the model.
Whatever that CLI sends to its provider is outside this app's control.
Treat the agent as a second trust boundary: only process days you are
willing to put through that tool, and read the ledger when a run looks
wrong.

MCP tools can read days, the KB, the ledger and settings, and can queue
jobs, while the app is running (and some reads work with the app closed).
Only enable clients you trust on that machine.

## Claims matrix

| Claim | Evidence level | Boundary |
| --- | --- | --- |
| No screenshot / video / OCR path in capture | Structural (source) | Source review of 1.0.0, not a notarised-binary audit |
| Focused window only; nothing while screen locked | Structural + platform | Depends on macOS frontmost and lock reporting |
| Secure fields skipped at AX walk | Structural | Depends on apps exposing `AXSecureTextField` correctly |
| Password managers / private titles dropped | Best-effort heuristic | Name and title substring lists; unknown apps and normal browser windows are not covered |
| Pattern redaction of common secrets | Best-effort heuristic | Regexes miss novel formats; user extras help |
| No Ambient Context upload of the record | Structural | Updater and user-opened links are separate; synced folders and agent CLIs are yours |
| Own window headings-only; own day files skipped | Tested behaviour | Measured self-capture regression drove the rule |

## Known gaps

- Banking, health and mail in an ordinary (non-private) browser window are
  captured like any other page or message surface.
- Apps that put secrets in ordinary text fields, or that mis-label secure
  fields, bypass the strongest control.
- Redaction does not remove secrets from `file:` / `url:` references when
  the path or query string itself is sensitive.
- Summaries and KB files can restate content that already passed into
  `Days/`. Deleting derivatives does not remove the day files.
- Ledger and rejected-output folders can hold model text about your day;
  treat them like the record when filing a bug (see `docs/bug-reports.md`).
- This is a source-backed description, not an independent security review.

## Reporting a problem

If you find a hole, open an issue on
[GitHub](https://github.com/dragthelake/ambient-context/issues). Prefer a
minimal reproduction over attaching real day files. Guidance on what to
include is in `docs/bug-reports.md`.
