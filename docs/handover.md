# Ambient Context: start here

Written for someone picking this up cold. It says where the work stands, what
is already decided, and which traps in this codebase will cost a session if
you meet them without warning.

The two older handovers are still worth reading and are not superseded:
`handover-v1.md` records the original unattended build, and
`handover-v1-ui.md` is the live record of the window's look, its
measurements, and its open visual items.

## What the app is

An app that writes a record of what you worked on: one markdown file per day, on your own computer. It reads the focused window through the accessibility API, and never takes screenshots. There is no account and no server, and that claim is on the About screen, so any
feature that phones home contradicts the product.

Once a day it can hand the day's file to an agent CLI already on the machine
(Claude Code, Codex, opencode) and save the summary next to the record. That
run happens under the user's own subscription. Nothing about it is a service.

An MCP surface lets clients read days, queue summaries and edit rules over a
unix socket. `docs/mcp.md` is its contract.

## Where things stand

Branch `v1`, **31 commits ahead of `origin/v1` and never pushed**. Working
tree clean. Every CI gate passes: `cargo fmt --check`, `cargo clippy
--all-targets -- -D warnings`, `cargo test`, `npx tsc --noEmit`,
`npx vitest run` (50 tests), `npm run build`.

Version is `0.1.0` with no git tags. Nothing has been released, which is why
the Agent plan takes a breaking MCP rename now rather than later.

### Built and merged

The **Overview defrag map**: a Windows 98 Disk Defragmenter field, one cell
per recorded day, that batch-summarises everything holding raw context with
no summary. Spec, plan and the reference capture are under
`docs/superpowers/`. `handover-v1-ui.md` carries its measurements and the two
findings deferred from its review.

The **calendar rail is gone** from the Context view. Day navigation is the
header's previous, next and Today, plus the map, which reaches any recorded
day in one click.

### Specced and planned, not built

The **Agent tab**. Renames "engine" to "agent", gives it its own tab, and
prompts from the Overview when none is connected.

- Spec: `docs/superpowers/specs/2026-09-01-agent-tab-design.md`
- Plan: `docs/superpowers/plans/2026-09-01-agent-tab.md`

Approved and ready to execute. The execution approach was never chosen.

### Designed in conversation, not written down

A **bug report flow**: a free-text field in Settings plus a status bar entry
point, offering Copy and Open a GitHub issue. Deliberately no automatic
diagnostics beyond the app version, because agent stderr and the ledger can
both carry the user's own content. Blocked only on GitHub Issues being
enabled on `dragthelake/ambient-context`, which is public.

## Decisions already taken, so they do not need relitigating

- **The word is "Agent", not "AI Agent"**, in copy and in code. MCP callers
  are "clients", so the word is not overloaded.
- **Three names keep the old word "engine"**, all persisted in the user's
  capture folder: the ledger's `engine` field, its `"engine_test"` action
  string, and the test asserting that string. Renaming them would rewrite
  records the app promised to write once.
- **The MCP error code `no_engine` becomes `no_agent`**, breaking any client
  matching the old one. Taken deliberately at 0.1.0 with nothing released.
- **The defrag map's Summarise button is uncapped** and carries the day count
  in its label, because each day is one agent run on the user's own
  subscription. `MAX_BACKFILL_DAYS` still caps the scheduled path at seven;
  that asymmetry is intended.
- **A failed day in a batch does not stop the batch.** The scheduled path
  stops on first failure, on the grounds that every later day would fail the
  same way. The manual path does not, because the user pressed the button and
  can see each result.

## Traps in this codebase

Each of these cost real time. They are not hypothetical.

**The cascade hazard.** `src/setup.css` and `src/main-window.css` share one
namespace and `main-window.css` is imported first, so setup.css wins at equal
specificity. `src/test/css-cascade.test.ts` guards identical selectors across
both files, and cannot see different selectors matching the same element. The
generic `button:active:not(:disabled):not(.titlebar-button):not(.tab)` sits
at specificity (0,4,1) and will outrank any new single-class button rule.
Add your class to that `:not()` chain, as `.defrag-cell` does.

**Dropping a property is not the same as setting it off.** Removing
`box-shadow` from a rule lets the generic `button` rule's four-layer
`--bevel-out` through. On a circle it renders as a smear. Set it to `none`
explicitly.

**Windows are visible before they paint.** A Tauri window is shown the moment
it is built and an unpainted webview is white. About is fixed with
`.visible(false)` plus `on_page_load`; the main and setup windows still
flash.

**jsdom has no ResizeObserver.** `src/test/setup.ts` stubs it through
`setupFiles`. Any component that measures itself needs that stub, not a
per-file one and not a runtime guard in the component.

**A `void`ed invoke swallows the test mock's throw.** `src/test/tauri-mock.ts`
is built so an unnamed command throws loudly, but a `void invoke(...)` call
site turns that into an *unhandled rejection*: vitest still reports every test
as passing. Read the "Unhandled Errors" block, not just the pass count.

**`tauri dev` does not always swap in a rebuilt binary.** If you build
manually while it is running, the process on screen can be older than the
binary on disk. Check the pid changed before believing a runtime test.

**`screencapture -l` captures a buried window.** The image looks correct
while every synthetic click lands on whatever is actually on top. Raise the
app first and confirm it, then validate the frame before trusting it:

```bash
osascript -e 'tell application "System Events" to set frontmost of \
  (first process whose name is "ambient-context") to true'
```

The validation is in `handover-v1-ui.md`: the title bar navy must be present.

## Environment

Node is pinned to **24.20.0** through Volta, in `package.json`. Both CI
workflows match.

Two things are unresolved and need a decision:

- `@types/node` is `^26`, so TypeScript accepts APIs the 24 runtime does not
  have.
- A root-owned Node 22.17.0 sits at `/usr/local/bin/node` and shadows Volta
  in any shell that does not source `.zshrc`, which includes scripts and
  launchd jobs. Removing it needs `sudo`.

The app is `LSUIElement` and only shows in the Dock while a window is open.
`tauri dev` runs a bare binary with no bundle, so anything about the app icon
has to be checked against `npm run tauri build`.

To open the main window without clicking the menu bar:

```bash
python3 -c "
import socket, json, os
p=os.path.expanduser('~/Library/Application Support/com.0x0000007a.ambientcontext/control.sock')
s=socket.socket(socket.AF_UNIX, socket.SOCK_STREAM); s.connect(p)
s.sendall((json.dumps({'op':'open_day','date':'2026-09-01'})+'\n').encode())
print(s.makefile().readline())"
```

## What is unverified

Green tests here say the logic holds. They say nothing about the window.
Two things have never been seen by anyone:

1. `.defrag-cell:focus-visible`. Synthetic Tab presses do not reach the grid.
   The ring is white then black drawn outside the cell, and both the well and
   the gaps between cells are white, so the white half may be invisible and
   the black half may merge with the neighbouring outlines.
2. The map's empty-state line, which shows only with zero recorded days.

`handover-v1-ui.md` holds the full open list, including the two performance
and keyboard findings deferred from the map's review.

## Process artefacts

Specs and plans live under `docs/superpowers/`. They are committed
deliberately: when something goes wrong they are the only record of why a
choice was made.

`.superpowers/sdd/2026-09-01-defrag-overview/` is a git-ignored working
directory from the map's subagent run. It holds the ledger, every task brief,
every implementer report and every review package. It is kept because that
plan's final visual task is not finished. Delete it once it is.
