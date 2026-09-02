# Changelog

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
- The systematic coverage census (which apps come back rich, partial or
  empty, and what always-on Chromium accessibility costs in CPU and
  memory) has not been run yet. `docs/census.md` has the template.
