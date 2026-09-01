# v1 UI and UX pass: state and open items

Branch `v1`. Written to carry state across a context compaction, for a
reader who was not in the session.

## What the main window is now

A Windows 98 tabbed dialog. Three tabs today, four once the Agent tab
lands: Overview, Context, Settings. The
tab strip, title bar, status bar and scrollbars are shared with the setup
dialog through `src/setup.css`, which `src/main-window.css` imports.

- **Overview** holds the eye and the record toggle, plus the defrag map.
- **Context** is the former Day view, without its calendar rail. Day
  navigation is the header's previous, next and Today, plus the Overview
  map, which reaches any recorded day in one click.
- **Settings** is the former stack of settings panels, plus a Sound
  section. The agent options leave it when the Agent tab lands.

The menu bar eye icon opens this window; it is no longer a capture toggle.
Capture is toggled from the tray menu, the Overview tab or MCP, all through
`tray::toggle_capture`.

## Reference material

Two screenshots drive the look. Both are Windows 98 Display Properties.

- `~/Dropbox/Cameron.library/images/MTHQR3WVYK7SH.info/desktop01.gif`
  Tabs, palette, title bar and its buttons.
- `~/Dropbox/Cameron.library/images/MTHQLAX7LVBWR.info/14fig02.jpg`
  The CRT monitor the eye sits in.

Measurements taken from them, all recorded in CSS comments beside the
values they justify:

- The palette is the VGA system palette, sampled from the GIF's indexed
  colour table rather than eyeballed: face `#c0c0c0`, highlight `#ffffff`,
  shadow `#808080`, dark shadow `#000000`, title `#000080`, desktop
  `#008080`. That dialog's title bar is flat, not a gradient.
- Title bar 18px against 11px type, scaled here to 22px against 13px type.
- Title bar buttons 16x14, inset 2px from the window edge.
- Tab height 18px, the current tab 2px taller.

## macOS geometry

The main window keeps native decorations, so macOS masks it to a rounded
rect. The radius was measured, not guessed: flatten the chrome to one flat
colour, screenshot the corner at 2x, and read the first opaque pixel per
row. On macOS 26 that fits a circle of **16pt** to within half a point.
Re-measure if the corners ever look cut on a newer macOS; the method is in
the comment on `.window.main-window`.

Inner chrome is stepped down by the border width so the arcs are
concentric: `--corner-inner-* = --corner - --border`. Top and bottom are
separate variables because the title bar meets the frame as a hard
saturated edge and needs a touch less than the arithmetic gives.

The traffic lights are hidden by `ambient_hide_window_buttons` in the Swift
plugin, called from `src-tauri/src/titlebar.rs`. Going borderless would
have hidden them too but would also have given up the corner mask and edge
resizing.

## The app icon

`src/assets/app-icon.png` is the artwork: a Windows 98 raised panel, square,
bled to the canvas edge. Clipping that square to the macOS squircle cuts the
corners straight through the white highlight band, so the bevel is redrawn to
follow the squircle instead and the eye is keyed out and dropped inside it.
`tools/make-app-icon.py` does this and writes `src/assets/app-icon-macos.png`,
which is what `tauri icon` is then pointed at. Both the script and its output
are committed; the script's docstring carries the commands.

- Apple's icon grid: a 1024 canvas with the art on an 824 square, so the icon
  carries the same visual weight as its Dock neighbours.
- The corner is a superellipse with exponent 5, which tracks Apple's
  continuous corner far more closely than a circular-arc rounded rect.
- The bevel tones are sampled from the artwork, not from `--bevel-out`. On a
  window the near-black is one hairline against a grey desktop; here it is a
  26px arc on transparency, and pure black reads as a drop shadow. The bands
  are unequal for the same reason the artwork's are: an even white band turns
  into a halo and the icon reads as an outline sticker.
- The light and dark sides meet on the anti-diagonal, which is the Windows
  miter rule, and it transfers to a superellipse unchanged.

`tauri icon` also writes `android/` and `ios/` trees. This app is macOS only;
delete them.

The app is `LSUIElement` and only promotes to `ActivationPolicy::Regular`
while a window is open, so the Dock icon exists but only while the window
does. `tauri dev` runs a bare binary with no bundle, so the icon can only be
checked against `npm run tauri build`. To check it without trusting the file,
ask macOS what it resolves for the bundle, which goes through the same icon
services cache the Dock uses:

```swift
NSWorkspace.shared.icon(forFile: "…/Ambient Context.app")
```

`docs/app-icon-dock-compare.png` is that render beside Mail, Notes and System
Settings at 256px, which is how the size and corner were settled.

## The Overview defrag map

The Status group is gone. In its place is a Windows 98 Disk Defragmenter
field, one cell per recorded day, with a status line, a segmented progress
bar and buttons that batch-summarise every day holding raw context and no
summary. Clicking a cell opens that day on the Context tab.

Spec at `docs/superpowers/specs/2026-09-01-defrag-overview-design.md`, plan
at `docs/superpowers/plans/2026-09-01-defrag-overview.md`, reference capture
at `docs/reference/defrag98-idle.png`.

- **Cell geometry is read, not inferred.** The pixel run across a boundary
  in the reference, at 2x: white 2, black 2, fill 12, black 2. Halved, a 1px
  outline around a 6x9 fill with a **1px white gap** to the next cell. The
  first attempt measured the pitch (9x12, correct) and assumed the outlines
  touched, which fused a run of summarised days into one navy slab. Box size
  and pitch are separate constants because they are no longer the same
  number: n columns occupy n boxes plus n-1 gaps.
- **The well needs 4px of padding, not 2.** Two go to the inset bevel, which
  consumes them entirely, and two are white. The clearance that appears at
  the sides is not padding: it is leftover width from centring the grid, and
  there is no vertical equivalent, so the field sat flush top and bottom.
- **A pressed cell fills with its outline colour.** Inverting the outline to
  white was tried and does not work: the well behind is white, so the outline
  vanishes and the cell appears to swell into its gap.
- **Cancelling a batch is a generation counter, not a flag.** `drain_if_idle`
  takes the whole queue into a local vector the moment the runner is idle, so
  clearing the queue stops nothing once a batch has started. `cancel_queued`
  bumps a counter; the runner snapshots it at drain and compares before each
  job. A flag could not tell "raised while idle" from "raised for the batch
  in flight".

Two findings are deferred, both real:

1. Every hover rebuilds `buildCells` and re-renders the field. The floor is
   `MIN_ROWS` times the column count, about 2000 buttons even for a user with
   one recorded day.
2. Every recorded day is a tab stop, ahead of the Legend and Summarise
   buttons. After a year that is 365 presses to reach them.

## The Agent tab, planned but not built

Renames "engine" to "agent" everywhere except two persisted names, gives it
its own tab, and prompts from the Overview when none is connected. Spec at
`docs/superpowers/specs/2026-09-01-agent-tab-design.md`, plan at
`docs/superpowers/plans/2026-09-01-agent-tab.md`.

The load-bearing detail: `Settings` carries `#[serde(default)]`, so renaming
the `engine` key would not fail, it would silently parse an existing
`settings.json` to "no agent" and discard the user's configuration. A
`#[serde(alias = "engine")]` reads either key and writes the new one.

Three names keep the old word, all persisted in the user's capture folder:
the ledger's `engine` field, its `"engine_test"` action string, and the test
asserting that string.

## Windows are shown before they paint

A Tauri window is visible the moment it is built, and an unpainted webview is
white. Opening About flashed a white rectangle: measured at 98.8% white on
the frame after the click, against 1% once painted.

About is now built with `.visible(false)` and shown from `on_page_load`.
Doing that in Rust rather than from the page keeps its capability as narrow
as it was made: it carries only `core:default` and `core:window:allow-close`,
and `show()` from the frontend would have needed `core:window:allow-show`.

**The main and setup windows are built the same way and still flash.**

## The cascade hazard

`src/setup.css` and `src/main-window.css` share one namespace, and
`App.tsx` imports `Main` (and so `main-window.css`) *before* `setup.css`.
So at equal specificity setup.css wins on source order. Four bugs came from
this, two on order and two on specificity:

- `.window` padding beating `.main-window` padding
- `body` background beating the main window's own
- `button:active:not(...)` beating `.tab:active`, which made tabs shift the
  whole pane on mousedown
- a stray `padding: 0` left at the end of the `.tabpane` rule, silently
  killing the pane's inset

**The sweep has been done**, and `src/test/css-cascade.test.ts` now guards
it. That test parses both files and fails on the two shapes that are always
wrong: the same property set on an identical selector in both files, and a
property set twice on one selector within a file. It reads the CSS from
disk deliberately, because vitest stubs CSS imports and a version built on
`import "./x.css?raw"` gets an empty string and passes while checking
nothing.

What the sweep could not settle is selectors that differ in text but match
the same element, which is how the `button:active` bug got in, and which
later put a `■` on every engine radio and rule row: `li::before` in setup.css
against `.engine-list` and `.rule-row` in main-window.css. The bullet is now
scoped to `ul:not([class])`, so prose lists keep it and control lists, which
all carry a class, do not.

That class of bug does need a DOM, but it does not need a person. Against the
dev server, with both sheets loaded, `getComputedStyle(el, "::before")` settles
it outright:

```js
// prose  → content "■", padding-left 14px
// engine → content none, padding-left 0px
```

There is no harness for that yet, because it wants a browser and a running
`vite`, which the vitest suite has neither of.

## Verification kit

Synthetic input through System Events is unreliable against this app, and
AppleScript `click` cannot show a `:active` state at all. Three throwaway
Swift helpers were written to post real HID events. They live in `/tmp` and
are **not** in the repo; worth keeping if this continues:

- `press.swift x y` holds the mouse down, so `:active` can be photographed
- `press2.swift x y` holds, then releases 300px away. A mouseup off the
  element cancels the click, which is the only way to photograph `:active`
  on a button that does something: holding the title bar `?` and releasing
  in place opens the About window every time
- `click.swift x y` a real down/up click, where System Events failed
- `scroll.swift x y ticks` real scroll wheel events

`screencapture -x -o -l<id>` replaces them for capture: `CGWindowListCreateImage`
was obsoleted in macOS 15, so the window id is read from `CGWindowListCopyWindowInfo`
and the capture is left to the command line tool.

Screenshots need a validation loop: raise the window, capture, and check
the title bar navy is present before trusting the image. Several captures
during the session silently grabbed the wrong window.

The same trap catches clicks, and more quietly. `-l` captures a window even
when it is buried, so a capture can look right while every click has been
landing on whatever is actually on top. Raise the app first and confirm it:

```
osascript -e 'tell application "System Events" to set frontmost of \
  (first process whose name is "ambient-context") to true'
```

## Open items

1. **`.defrag-cell:focus-visible` has never been seen.** Synthetic Tab
   presses do not reach the grid, so the ring was never photographed. The
   risk is specific: it is `0 0 0 1px #ffffff, 0 0 0 2px #0a0a0a` drawn
   outside a cell, and the well and the inter-cell gaps are both white, so
   the white half may be invisible and the black half may merge with the
   neighbouring cells' outlines rather than reading as a ring.
2. **The empty-state line has never been seen.** It shows only with zero
   recorded days, which no development machine here has.
3. **The main and setup windows still flash white** before they paint. Same
   defect as About, same three-line fix.
4. **The map's two deferred findings**, above: the hover re-render and the
   tab stops.
5. **`@types/node` is `^26` against a Node 24 runtime**, so TypeScript
   accepts APIs the runtime does not have.
6. **A root-owned Node 22.17.0 at `/usr/local/bin/node`** shadows Volta in
   any shell that does not source `.zshrc`, which includes scripts and
   launchd jobs.

## Since fixed

- **Sound is verified by ear.** `cuelume` plays `ready` on starting capture,
  `release` on stopping, `tick` on changing tab and `chime` on opening About.
  Volume and on/off are in Settings.
- The title bar `?` and `×` are circles: a flat `--chrome` disc with a 1px
  `--chrome-darker` outline, darkening to `--chrome-dark` when pressed. Flat
  because no bevel survives at 16px on a circle. An offset inset shadow
  displaces the interior instead of making a ring, which is the same reason
  the Settings radios are SVG arcs; a conic gradient painting a two-tone ring
  notches where its light and dark halves meet and goes uneven in thickness
  around the curve; and a radial gradient cannot size a 1px ring at all,
  because its percentage colour stops measure along the ray to the farthest
  corner rather than the radius, so a nominal 1px renders at 3px and soft.
  Both were built and photographed before being discarded.
- `box-shadow: none` on `.titlebar-button` is deliberate and must stay.
  Dropping the property rather than setting it to none lets the generic
  `button` rule's four-layer `--bevel-out` through, and on a circle that
  reads as a smear. It cost an hour; `getComputedStyle` found it in one call
  after three screenshots had not.
- The pressed state no longer nudges the glyph. `padding: 1px 0 0 1px` on a
  fixed-height button only ever moved it sideways: centring inside a content
  box one pixel shorter resolves to the same row. Measured against a real HID
  press at +1px horizontal, 0 vertical.
- The close button no longer needs its top right corner rounded to clear the
  window's corner arc. A circle has nothing to shave.
- **The app icon** has its own section above.
- `.raw-pane` and `.summary-pane` had the same scroll-layer defect as
  `.tabpane`: an inset bevel painted under scrolling content. Both are now
  a non-scrolling frame around a scrolling child. `SummaryPane` had five
  separate returns of the same markup; they go through one local `Pane`
  component now.
- The About window has its own capability with only `core:default` and
  `core:window:allow-close`, rather than inheriting the default set.
- Title bar glyphs are traced bitmaps in `src/components/PixelGlyph.tsx`.
  The buttons that carry them started as the reference's 16x14 rectangles
  with its own simpler bevel; they are circles now, as above.
- `--bevel-in` had its two dark tones inverted. Windows puts the mid grey
  outside and the black inside; ours was the other way round, which made
  every sunken surface in the app read as drawn on rather than cut in.
- Checkboxes and radios were still native macOS controls. Both are now
  drawn: the checkbox from the reference's own 9x7 tick bitmap, the radio
  from SVG arcs, because an offset inset shadow on a circle displaces the
  interior instead of making a concentric ring.
