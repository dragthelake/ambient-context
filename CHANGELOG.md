# Changelog

## Unreleased

### Capture

- **Chinese and Japanese lines are no longer dropped as UI labels.** The
  short-fragment filter counted whitespace-separated words, and text
  written without spaces between words is one token however long it is,
  so nearly every Chinese or Japanese sentence was discarded. A line with
  at least six Han ideographs or kana is now kept; shorter ones such as
  「保存」 or 「打开文件」 are still dropped like their English equivalents.
### Privacy

- **Apple Passwords and the system password prompt are never read.**
  `Passwords` (the macOS 15 app) and `SecurityAgent` (the process that
  draws authentication and keychain dialogs) join the built-in excluded
  applications, so their windows are dropped before anything is written.

## 1.1.3

Fixes daily schedule settings reverting instead of saving.

### Agent settings

- **Daily schedule controls now persist their changes.** The schedule
  checkbox and time field capture their values before the asynchronous
  settings save, so a controlled re-render cannot restore the old value
  before it is written.

## 1.1.2

The login item follows the installed app, and a crash at launch leaves
its message on disk.

### Launch at login

- **The login item is checked against the running app, not just for
  existence.** Turning launch at login on from a build under `target/`
  wrote a login item naming that binary, and every login since started
  the stale dev build beside the installed app because the startup check
  only asked whether a login item existed. The check now reads the
  program the login item names and rewrites it when an installed app
  finds it pointing elsewhere. A build that is not under an Applications
  folder never writes a login item.

### Diagnostics

- **`panic.log`** in the app's data folder. A panic during startup runs
  inside a system callback that cannot unwind, so the process aborts and
  the crash report records only the abort. The message now goes to
  `panic.log` first, with the thread and source location.

## 1.1.1

Fixes a crash when the app's own window was brought to the front while
capture was running.

### Capture

- **No self-capture crash.** The accessibility snapshot never reads this
  app's own window. A query on the calling process is answered inside
  AppKit on the poll thread rather than over IPC, and AppKit trapped on it
  ("Must only be used from the main thread") the moment a poll landed just
  after a Dock click had shown the window. The poll now skips its own
  window outright, which also closes the open block, the same as a locked
  screen: looking at Ambient Context is not work to record.

## 1.1.0

Five agent skills ship inside the app and install with one click.

### Agent skills

- **Five skills** in the binary: `ambient-context` (the core loop: what the
  record is, which tool answers which question, restraint, privacy),
  `ambient-context-catch-me-up`, `ambient-context-standup`,
  `ambient-context-weekly-review` and `ambient-context-tune-rules`. The
  source is `skills/` in the repository, so
  `npx skills add dragthelake/ambient-context` works too.
- **Settings > Agent skills:** Install writes them to `~/.claude/skills/`
  and `~/.agents/skills/`, which Claude Code, Cursor, Codex, Zed, Copilot,
  Gemini CLI, Goose and OpenCode read. Update appears when a release changes
  a skill. Remove deletes only what the app wrote. A `SKILL.md` you have
  edited is never overwritten or deleted without a click on Replace my
  edits.
- **Overview:** an Install agent skills button above Star on GitHub until
  they are installed, and Update agent skills when a newer one ships.
- **Ledger:** when a capture folder is set, every install, update and remove
  is written to the day's ledger with the paths.
- **Tests** hold every skill to the real tool surface: names, descriptions,
  declared tools, tools named in the body, and `docs/skills.md`.

### Docs

- `docs/skills.md` covers each skill, the install paths, the npx route and
  what happens to edited files. The privacy document lists the two
  directories the app now writes outside its own.

## 1.0.1

The app now updates itself. A 1.0.0 install has no updater and must be
replaced by hand with this release; from 1.0.1 on, a new version arrives
as a notification and a menu bar item.

### Automatic updates

- **Background check** thirty seconds after launch and every six hours,
  on its own thread so it runs whether or not a folder or agent is set
  up. Offline or a GitHub error is logged and never shown.
- **One notification per version per launch:** "Version X is available.
  Update from the menu bar."
- **Menu bar:** a standing **Check for Updates…** item under About…. While
  a version is held it reads **Update to X…**; clicking it downloads,
  verifies the signature, installs, flushes the open capture block the way
  Quit does, and relaunches. During the install it reads **Installing X…**
  and is disabled. With nothing held it runs the check and reports the
  outcome.
- **About window:** a **Check for updates** button that runs the same
  check with its dialogs.
- **Setting:** Check for updates automatically, on by default, under
  Application; also `check_updates` over MCP `set_config`. Off leaves the
  manual paths only.
- The privacy document's network table says when the check runs and what
  it sends.

### Release and docs

- **Disk image renamed.** Releases carry `Ambient-Context-<version>.dmg`
  and a fixed-name `Ambient-Context.dmg`, so a direct download link stays
  valid from one release to the next.
- **README rewritten** around why the record exists, with product
  screenshots, the 1.0.0 demo, and a download button above the fold.

## 1.0.0

First public release. The 0.2.0 pipeline (per-day folders, a cited
knowledge base, a summary written from it) is unchanged underneath. What
changed is how it is presented and run: the day view now names the three
things the app produces, Context, Knowledge and Notes, and every run is a
single button whose label says what it will do. Claude Code can use one
model to build the knowledge and another to write the notes. Runs report
their outcome as a notification. The privacy wording is tighter, and
capture and the readers are hardened against records that mislead the
model.

### Product

- **Day view:** navigation on top, then three tabs, Context (the record),
  Knowledge (the wiki) and Notes (the written day), then the content box,
  then the actions underneath it. Context keeps its Apps / Websites /
  Messages strip; Knowledge gets one for People, Commitments, Threads,
  Products, Issues and Reading, one section at a time. Nothing in the header
  moves when a strip appears.
- **One action per tab.** Process day (or Reprocess day) on Context runs the
  full pipeline. Generate (or Regenerate) on Knowledge builds only the
  wiki; on Notes it writes the day, building the knowledge first if it is
  missing. Empty states say what Generate will build. Summarise, Ingest and
  Re-ingest are gone as words and as buttons.
- **Overview:** two columns, CRT and controls on the left, the Record map
  on the right. The batch button reads Process N days; the legend and map
  say Recorded and Processed. A Notes list under the map links into
  processed days by their title, with empty states when nothing is
  recorded or nothing is processed yet. Star on GitHub and Report a bug
  sit at the foot of the left column.
- **Notifications:** every batch ends with one notice, whether it was
  scheduled, started from the window or triggered over MCP. A single run
  names its day and how long it took; several report counts and the first
  failure. A success is quiet while the window is focused; a failure never
  is.
- **Agent settings:** Claude Code offers a Context model for the knowledge
  calls and a Notes model for the written day, and both lists include
  Fable 5.1, so the long input can go to a cheaper model than the final
  page. The separate ingest agent picker is gone. The schedule row reads
  Process each day at.
- **Highlight pill:** anchored to the selection as it moves. It stays inside
  the pane at either edge, drops below the selection when there is no room
  above, follows a scroll, and hides when the text scrolls out of view. It
  opens on mouseup rather than during the drag.
- **About:** fuller product description, open-source section with GitHub
  links, and a clearer Built by credit. The tray no longer offers Open
  Today's File (open a day from Overview or Context instead).
- **No interface sounds:** cuelume and the Sound settings are removed.

### Privacy and docs

- **Claims tightened** in the README and Setup window: no Ambient Context
  upload of the record, updater / agent CLI / synced folders called out as
  separate boundaries, redaction described as defence in depth.
- **`docs/privacy-and-security.md`:** trust boundary, data inventory,
  control layers, network side channels, claims matrix and known gaps for
  1.0.0.
- **`docs/bug-reports.md`:** what to include (and what not to) when filing
  an issue.

### Capture and pipeline (since 0.2.0)

- Idle input closes the open block and stops recording after the configured
  quiet period.
- Replayed records (earlier days opened again) are marked at block close;
  tracking parameters are stripped from visit URLs.
- Summaries whose citations or figures are not in the evidence are
  rejected.
- A body line that looks like a block heading cannot forge a block. The
  writer escapes such lines, and the readers only accept a heading in
  heading position, so a record captured before the escape is read the same
  way. The text handed to the ingest calls is escaped as well.
- Redaction patterns are validated on save; the editor setting and login-
  item errors are surfaced in Settings.
- Run durations are recorded in the ledger and shown in the day view;
  rejected agent output is kept for debugging.
- Whole-word app matching for exclusions; KB presence checked on disk;
  websites pane framing fixed.

## 0.2.0

Per-day folders, a daily knowledge base, and summaries that read cited
evidence instead of raw bodies.

- **Days/ layout:** each day is `Days/YYYY-MM-DD/apps.md` (timeline with
  native app bodies), `websites.md` (visit table, no bodies) and
  `messages.md` (routed message bodies).
- **Message routing:** built-in table plus user `route_messages` rules send
  mail and chat bodies to `messages.md`.
- **Own-window capture:** the Ambient Context window and KB files record as
  headings only.
- **Knowledge base:** three ingest calls (messages, apps, websites) write six
  cited files under `KB/YYYY-MM-DD/` plus `manifest.md`.
- **Summary from KB:** the daily summary prompt receives timeline headings
  and the KB, not full raw bodies.
- **Day view:** Raw tabs, KB pane with Ingest and Re-ingest, job step text.
- **Settings:** separate ingest agent and ingest input cap; prompt selector
  for all four prompts.
- **MCP:** `read_kb` and `ingest_day` (twenty tools total).
- **0.1 flat day files** (`YYYY-MM-DD.md` at the folder root) are no
  longer read.

---

Dated entries below predate versioning. Nothing had been tagged yet when
0.2.0 shipped.

## 2026-08-26

**Noise reduction, measured on real days.** Two full days of my own capture
(about 12,500 lines) showed that 22% of everything recorded was bare
counters and social chrome: view counts, vote counts, "8 minutes ago",
media player positions ticking every poll. Worse, lines that differ only in
a number (the same tweet with its ago-counter ticked) slipped past
deduplication and cost about 26k tokens across the two days. Capture now
drops counter-shaped lines and pipe-separated navigation menus, normalises
non-breaking spaces so identical lines deduplicate properly, and recognises
digit-varying re-captures at both block and day scope. Short identifiers
(ticket numbers, versions, dates) are explicitly protected. Net effect on
those two days: roughly 10% smaller output on top of the existing dedup,
with no real content lost in sampling.

**A prompt for turning a day into context.** `docs/day-context-prompt.md`
distils a captured day into a compact summary another LLM can work from:
timeline sessions, work and outcomes, open loops, and a capped
"worth remembering" list. Treat it as a starting point.

**Licences.** The project is now MIT. The bundled Funnel fonts carry their
SIL OFL text.

**Fixed: changing the capture folder while recording did nothing.** The
capture loop read the folder once at start. It now picks up a folder change
within one poll, flushes the open block to the old folder first, and starts
fresh in the new one. Deleting a day file now means "start over" rather
than getting a hollow file of bare headings back.

**Fixed: granting Accessibility mid-session did not start recording.**
Auto-start only ran at launch, so granting permission while the app was
open left it idle until a relaunch. The settings page now starts recording
the moment the grant arrives.

## 2026-08-25

First working version, built and iterated in a day.

**What it does.** Reads the focused window's text via the Accessibility API
every few seconds and appends it to one markdown file per day. Each block
records its time range, app, window title, and where possible the backing
file path or page URL, so an LLM reading the record can open the real
document instead of trusting scraped fragments. Lines are deduplicated
across the day. The capture folder gets an AGENTS.md explaining the format
to whatever reads it.

**Privacy posture.** No network calls at all in this build. Password
managers and private browsing windows are never captured, secure fields are
skipped at the accessibility level, and credential-shaped strings are
scrubbed before writing. Only the focused window is read, never while the
screen is locked, and the capture folder is excluded from capture so the
app cannot observe its own output.

**Recording is the default.** Once permission is granted and a folder
chosen, capture starts with the app. Stopping it is remembered across
launches until you start again.

**The settings window** is a Windows 98 dialog with an animated ASCII eye
that is awake while recording and asleep while not. The menu bar icon is
the same eye, open or closed. This is not to everyone's taste. It is to
mine.

**Known issues, honestly:**

- Replacing the app with a new build silently kills the Accessibility
  grant, because unsigned builds get a new identity every time. Capture
  just stops. Fix: remove Ambient Context from System Settings, Privacy &
  Security, Accessibility, and grant it again. This bit me three times in
  one afternoon and will go away only when releases are signed.
- Chromium and Electron apps (Chrome, Slack, VS Code, Obsidian) build
  their accessibility tree only on first contact, so the first seconds of
  capture in those apps are thin.
- GPU-rendered terminals (Kitty, Alacritty) expose little or no text.
  Terminal.app and iTerm2 work.
