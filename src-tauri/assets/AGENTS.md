# Reading this folder

This folder is written by Ambient Context, a macOS menu bar app that records
what its user works on. While capture is switched on, it reads the text of
the focused window every few seconds and appends it here. You, the reading
LLM, are the intended audience: these files exist so you can build context
about the user's day without asking them to narrate it.

## Files

- `Days/YYYY-MM-DD/apps.md`: the timeline for one day, with native app bodies.
- `Days/YYYY-MM-DD/websites.md`: a visit table for websites (no bodies).
- `Days/YYYY-MM-DD/messages.md`: message bodies routed out of mail and chat.
- `KB/YYYY-MM-DD/`: six cited knowledge-base files plus `manifest.md`.
- `Summaries/YYYY-MM-DD.md`: one distilled account per day.
- `Ledger/YYYY-MM-DD.md`: every model action taken on the user's behalf.
- `AGENTS.md`: this file.

Flat `YYYY-MM-DD.md` files from older versions are ignored.

## Format of the day files

Each day folder holds three append-only files.

`apps.md` blocks look like this:

```
## 09:41–10:05 · Zed · ambient-context/src/lib.rs

file: /Users/x/project/src/lib.rs

<text lines new to this day>
```

Each `##` block is one stretch of attention: a window the user stayed in.
The heading carries the time range, the application, and the window title.
A `file:` line is the path of the document backing that window; a `url:`
line is the web page. Both are optional. A `routed: websites` or
`routed: messages` line on a block in `apps.md` means the block's content
went to that file instead: a visit row for a web page, or the body of a
message window.

`websites.md` is a pipe table with one row per visit, in time order:
`start | end | app | domain | title | url`. Bodies of web pages are never
recorded; the URL is the reference. Add up the rows for a URL to get its
dwell and visit count.

`messages.md` holds the bodies of routed message windows, with the same block
headings as `apps.md`.

## The knowledge base

`KB/YYYY-MM-DD/` holds six derived files: `people.md`, `commitments.md`,
`threads.md`, `products.md`, `issues.md` and `reading.md`, plus
`manifest.md` which records which ingest call produced each file and from
which input hashes.

The KB is regenerable from `Days/` alone. Treat it as structured evidence,
not primary record. Every line in the six files carries a time citation
(`HH:MM-HH:MM`). `Nothing evident.` means the ingest step found nothing to
say for that section, not that nothing happened.

## How to read it

- **Read the summary for what a day meant, the KB for structured evidence,
  and `Days/` for the record.** The summary interprets; the KB cites; the
  day files are what was actually captured.
- **The headings are the timeline.** Time range, app and title alone tell
  you most of what the day was. Read all headings first; read block bodies
  only where you need detail.
- **A block with no body is not empty activity.** Body lines are deduplicated
  across the whole day: a line is written the first time it is seen and
  never again. A bare heading means the user was there, looking at things
  already recorded earlier. Do not conclude "nothing happened".
- **Follow references instead of trusting fragments.** When a block has a
  `file:` or `url:` line, the real document is the source of truth and you
  can usually open it. The captured text under it is a noisy, partial
  scrape; the reference is exact.
- **The text is accessibility-tree scrape, not prose.** Lines arrive in
  visual order, mixed with residual interface text, and long documents
  appear only as the parts that were on screen. Treat it like OCR output:
  good for what was seen, silent about what was not.
- **`[redacted]` marks removed secrets.** Credentials, keys and card-shaped
  numbers are scrubbed before writing. Password managers and private
  browsing windows are never captured at all, so their absence is by
  design.
- **Be honest about gaps.** The user can stop capture at any time, so
  uncaptured hours are normal and mean nothing beyond "not recorded".

## Summaries

`Summaries/YYYY-MM-DD.md` holds one distilled account per day, written by
the user's own LLM from the day's timeline headings and knowledge base.

Read the summary for what a day meant, and the day files and KB for
evidence. The summary interprets and can be wrong; the record cannot.
Every claim in a summary carries the time range that supports it, so open
the day file at that range rather than trusting the sentence.

A day with no summary means the summary has not run yet, never that nothing
happened.

## Ledger

`Ledger/YYYY-MM-DD.md` records every time the summariser or ingest step
ran, including the runs that produced nothing. Each entry names the prompt,
the engine, the files that went in with their content hashes, what came out,
and why.

Ingest runs appear as `ingest_messages`, `ingest_apps` and `ingest_websites`.
The summary run is `summarise_day`. Read the ledger when a summary or KB
file looks wrong, or when a day has no summary and you need to know whether
that means it failed or has not run. The hashes let you tell whether an
input has changed since the output was written.

The reasoning in an entry is the model's own account of its choices. It is
not a record of what the model computed, and it can be wrong in the same
ways the summary can.

These files never leave this machine unless the user moves them. Treat them
with the discretion their existence assumes.
