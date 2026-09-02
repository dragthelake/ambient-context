# v1 UI and UX pass: state and open items

Branch `v1`. Written to carry state across a context compaction, for a
reader who was not in the session.

## What the main window is now

A Windows 98 tabbed dialog. Three tabs: Overview, Context, Settings. The
tab strip, title bar, status bar and scrollbars are shared with the setup
dialog through `src/setup.css`, which `src/main-window.css` imports.

- **Overview** holds the eye and the record toggle, plus a Status group.
- **Context** is the day view. Navigation on top, then three mode tabs,
  Context > Knowledge > Notes, then the content box, then the action row
  underneath it. Context has a second strip for Apps, Websites and
  Messages; Knowledge has one for its six sections. The first button in the
  action row is the tab's own: Process day (the whole pipeline) on Context,
  Generate on Knowledge (the three ingest calls only, `ingest_now`) and on
  Notes (the pipeline, which builds the knowledge first if it is missing).
  The window tab and the first mode tab are both called Context; that
  collision is open.
- **Settings** is the former stack of settings panels.

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

None outstanding for the original v1 UI pass. Interface sounds were removed
from the product.

## Since fixed

- `.raw-pane` and `.summary-pane` had the same scroll-layer defect as
  `.tabpane`: an inset bevel painted under scrolling content. Both are now
  a non-scrolling frame around a scrolling child. `SummaryPane` had five
  separate returns of the same markup; they go through one local `Pane`
  component now.
- The About window has its own capability with only `core:default` and
  `core:window:allow-close`, rather than inheriting the default set.
- Title bar glyphs are traced bitmaps in `src/components/PixelGlyph.tsx`,
  and the buttons use the reference's own simpler bevel at 16x14.
- `--bevel-in` had its two dark tones inverted. Windows puts the mid grey
  outside and the black inside; ours was the other way round, which made
  every sunken surface in the app read as drawn on rather than cut in.
- Checkboxes and radios were still native macOS controls. Both are now
  drawn: the checkbox from the reference's own 9x7 tick bitmap, the radio
  from SVG arcs, because an offset inset shadow on a circle displaces the
  interior instead of making a concentric ring.
