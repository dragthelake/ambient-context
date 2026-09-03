# Reading this folder

This folder is written by Ambient Context, a macOS menu bar app that records
what its user works on. While capture is switched on, it reads the text of
the focused window every few seconds and appends it here. You, the reading
LLM, are the intended audience: these files exist so you can build context
about the user's day without asking them to narrate it.

## Files

- `YYYY-MM-DD.md`: one file per day, blocks appended in real time.
- `Summaries/YYYY-MM-DD.md`: one distilled account per day.
- `Ledger/YYYY-MM-DD.md`: every model action taken on the user's behalf.
- `AGENTS.md`: this file.

## Format of a day file

```
---
date: 2026-08-25
captured_by: Ambient Context 0.1.0
---

## 09:41–10:05 · Chrome · Tauri tray documentation

url: https://v2.tauri.app/learn/system-tray/

<text lines new to this day>
```

Each `##` block is one stretch of attention: a window the user stayed in.
The heading carries the time range, the application, and the window title.
A `file:` line is the path of the document backing that window; a `url:`
line is the web page. Both are optional.

## How to read it

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

## Building context about the user

- Prefer patterns over incidents: which projects recur, which documents
  keep being reopened, what a normal morning looks like. One glance at a
  page rarely means anything; the fifth return to it does.
- Time spent is signal. A 40-minute block on one document outweighs twenty
  seconds on ten tabs, whatever the text volume says.
- Cross-reference days. The same ticket, path or name appearing across
  files is the thread worth pulling.
- When summarising a day for memory, lead with what the user worked on and
  decided, not what they merely saw. Meetings, documents written, and
  repeated returns to a problem are worth keeping; passing reads mostly
  are not.
- Be honest about gaps. The user can stop capture at any time, so
  uncaptured hours are normal and mean nothing beyond "not recorded".

## Summaries

`Summaries/YYYY-MM-DD.md` holds one distilled account per day, written by
the user's own LLM from the day file of the same name.

Read the summary for what a day meant, and the day file for evidence. The
summary interprets and can be wrong; the day file is the record and cannot.
Every claim in a summary carries the time range that supports it, so open
the day file at that range rather than trusting the sentence.

A day with no summary means the summary has not run yet, never that nothing
happened.

## Ledger

`Ledger/YYYY-MM-DD.md` records every time the summariser ran, including the
runs that produced nothing. Each entry names the prompt, the engine, the
files that went in with their content hashes, what came out, and why.

Read it when a summary looks wrong, or when a day has no summary and you
need to know whether that means it failed or has not run. The hashes let you
tell whether an input has changed since the summary was written.

The reasoning in an entry is the model's own account of its choices. It is
not a record of what the model computed, and it can be wrong in the same
ways the summary can.

These files never leave this machine unless the user moves them. Treat them
with the discretion their existence assumes.
