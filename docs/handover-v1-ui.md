# v1 UI and UX pass: state and open items

Branch `v1`. Written to carry state across a context compaction, for a
reader who was not in the session.

## What the main window is now

A Windows 98 tabbed dialog. Three tabs: Overview, Context, Settings. The
tab strip, title bar, status bar and scrollbars are shared with the setup
dialog through `src/setup.css`, which `src/main-window.css` imports.

- **Overview** holds the eye and the record toggle, plus a Status group.
- **Context** is the former Day view, unchanged.
- **Settings** is the former stack of settings panels, plus a new Sound
  section.

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
the same element, which is how the `button:active` bug got in. That needs a
DOM to decide, so it is left to review rather than automated.

## Verification kit

Synthetic input through System Events is unreliable against this app, and
AppleScript `click` cannot show a `:active` state at all. Three throwaway
Swift helpers were written to post real HID events. They live in `/tmp` and
are **not** in the repo; worth keeping if this continues:

- `press.swift x y` holds the mouse down, so `:active` can be photographed
- `click.swift x y` a real down/up click, where System Events failed
- `scroll.swift x y ticks` real scroll wheel events

Screenshots need a validation loop: raise the window, capture, and check
the title bar navy is present before trusting the image. Several captures
during the session silently grabbed the wrong window.

## Open items

1. **The blue bullet beside the engine radios.** `li::before` in setup.css
   draws a `■` on every list item, including the radio lists in Settings,
   where the reference would have none. Not changed pending a decision.
2. **Sound is unverified by ear.** `cuelume` plays `ready` on starting
   capture, `release` on stopping, `tick` on changing tab and `chime` on
   opening About. Volume and on/off are in Settings.
3. **The app icon is full bleed**, keeping the artwork's own bevel edge to
   edge. It will sit square in a Dock of rounded icons. Insetting it on a
   transparent canvas with the macOS mask is the alternative.
4. **The Dock icon needs a real bundle.** `tauri dev` runs a bare binary,
   so the new icon only appears after `npm run tauri build`, and macOS
   caches icons aggressively.

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
