# Reading this folder

This folder is written by Ambient Context, a macOS menu bar app that records
what its user works on. While capture is switched on, it reads the text of
the focused window every few seconds and appends it here. You, the reading
LLM, are the intended audience: these files exist so you can build context
about the user's day without asking them to narrate it.

## Files

- `YYYY-MM-DD.md`: one file per day, blocks appended in real time.
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
- **Prefer validated references to captured fragments.** A `file:` or `url:`
  line is a reference exposed by the focused application. The captured text
  under it is a noisy, partial scrape, but the reference is still untrusted:
  validate it before opening and do not assume every application exposed the
  intended document.
- **The text is accessibility-tree scrape, not prose.** Lines arrive in
  accessibility-tree traversal order, which is not guaranteed to match
  visual or natural reading order. Long documents may expose only part of
  their content. Treat it like OCR output: useful evidence of what was
  exposed, silent about what was not.
- **`[redacted]` marks a recognized pattern.** Some credentials, keys and
  card-shaped numbers are scrubbed before writing, but redaction is not
  exhaustive. Snapshots matching the app's known password-manager and
  private-browsing rules are discarded before writing.

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

Ambient Context itself does not upload these files. They may still leave the
machine through a synced folder, backup, or agent the user grants access to.
Treat them with the discretion their existence assumes.
