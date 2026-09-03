# Ambient Context

A macOS menu bar app that keeps a written record of what you work on, for
your own LLM to read.

<p align="center">
  <img src="docs/ambient-context.gif" width="520" alt="Ambient Context settings window, with the ASCII eye open while recording" />
</p>

While the eye in your menu bar is open, Ambient Context reads the text of
whichever window you have focused (via the macOS accessibility tree, every
few seconds) and appends it to plain markdown files in a folder you choose:
one day folder with apps, websites and messages. Point Claude Code or any
other agent at that folder and it can answer "what did I work on Tuesday?",
build memory about your projects, or write your standup for you.

- **No screenshots, no video.** Capture reads text through the accessibility
  API. There is no screenshot, screen-recording or OCR path.
- **No account and no upload of your record.** There is no Ambient Context
  server and no telemetry. Optional update checks hit GitHub Releases. If
  you connect an agent CLI, that tool runs on your machine under your own
  subscription. A synced capture folder (iCloud, Dropbox, …) is a separate
  boundary you choose.
- **Files you own.** Plain markdown in a folder you chose. Move them, grep
  them, delete them.
- **Defence in depth before writing.** Secure password fields are skipped
  at the accessibility source. Recognised password-manager apps and private
  browsing windows are dropped before a block is kept. Known credential,
  API-key and card-shaped patterns are scrubbed to `[redacted]`. These are
  heuristics, not a guarantee that every secret is removed.
- **Built to be read by an LLM.** Lines are deduplicated across the day,
  interface junk is filtered out, and blocks record a document path or URL
  where the focused app exposes one. The folder carries an `AGENTS.md`
  explaining the format to whatever reads it.

Requires macOS 14+ on Apple Silicon.

The full model, claims matrix and known gaps are in
[Privacy and security](docs/privacy-and-security.md).

## Status

Version **1.0.0**. There is no notarised download yet (Apple Developer
enrolment is in progress), so for now you build it yourself:

## Build and run

You need [Node](https://nodejs.org), [Rust](https://rustup.rs) and Xcode
Command Line Tools.

```bash
git clone https://github.com/dragthelake/ambient-context
cd ambient-context
npm install
npm run tauri build
```

The app lands in `src-tauri/target/release/bundle/macos/`. Drag
`Ambient Context.app` to Applications and open it.

For development, `npm run tauri dev` runs it with hot reload.

## First run

1. The settings window opens by itself. Grant Accessibility when asked:
   this is the permission that lets the app read window text, and nothing
   works without it.
2. Choose where to save. Prefer a folder outside iCloud Drive if you want
   the files to stay on this computer only.
3. That's it. Recording starts once setup is complete and starts with the
   app from then on. Click the eye in the menu bar to stop; stopping is
   remembered until you start again.

Open eye: recording. Closed eye: not. Right-click the icon for the
folder and settings.

## What a day looks like

Each day is a folder under `Days/YYYY-MM-DD/`:

- `apps.md`: the timeline, with native app bodies
- `websites.md`: visit rows (no page bodies)
- `messages.md`: routed mail and chat bodies

Optional derived files: `KB/YYYY-MM-DD/` (structured notes) and
`Summaries/YYYY-MM-DD.md`. `AGENTS.md` in the capture folder documents the
format and how to read it well.

## Notes for testers

- Chromium and Electron apps (Chrome, Slack, VS Code, Obsidian, Figma...)
  only build their accessibility tree when asked, so the first seconds of
  capture in those apps are thin and fill in on later passes. Chrome may
  show a slightly glitchy window-resize animation while enabled; that is a
  known cost of the mechanism.
- GPU-rendered terminals (Kitty, Alacritty) expose little or no text.
  Terminal.app and iTerm2 work.
- If an app comes back empty or thin, open an issue with the app name and
  what you were doing in it.

## Tests

```bash
cd src-tauri && cargo test
cd .. && npx tsc --noEmit && npx vitest run
```

## Privacy, short version

The app reads only the focused window, not background windows, and not
while the screen is locked. Secure fields, recognised password managers and
private browsing titles are filtered before writing; pattern redaction
covers common secrets. Output is plaintext on disk you control. See
[Privacy and security](docs/privacy-and-security.md) for the evidence and
the gaps. If you find a hole, please
[open an issue](https://github.com/dragthelake/ambient-context/issues).
