# Overview defrag map: design

Replaces the Status group on the Overview tab with a Windows 98 Disk
Defragmenter view of every recorded day, plus the controls to summarise the
days that have raw context but no summary yet.

## Why

The Status group repeats what the status bar along the bottom of the window
already says: whether capture is running, whether access is granted, and
which folder is being written to. It spends the largest panel on the tab on
three lines of text that are already on screen.

The defrag map spends it on the one thing nothing else shows: the shape of
the record over time, and which parts of it have been summarised.

## Reference

`~/Dropbox/Cameron.library/images/MTI5K8NWZOHPU.info/Screenshot - 2026-09-01 14.13.51.png`,
a Windows 98 Disk Defragmenter mid-run. `https://defrag98.com/` is a live
simulator of the same dialog.

Everything below is measured from that screenshot rather than estimated. It
is a 2x capture, so image pixels are halved to get CSS pixels.

- **Palette**, sampled from the image: empty `#ffffff`, light blue
  `#8be2f8`, dark blue `#0308a3`, accent red `#d84a3a`, cell outline
  `#0a0a0a`. The progress bar is `#00007b` on the `#c0c0c0` face.
- **Cells** are a 7x10 fill inside a 1px outline, so the pitch is 9x12.
  Adjacent outlines touch rather than leaving a gutter, which makes the
  grid read as one black lattice with coloured cells in it.
- **The reference field** is 91 columns by 53 rows.
- **Below the well**, in order: a status line, a segmented progress bar in
  a sunken trough, a percentage line, then a button row with two buttons at
  each end.

## The map

One cell per day, filling left to right and wrapping, oldest first. The run
starts at the first day with any recorded context and continues past today
into white cells, so the field is always a full rectangle.

| Colour | Meaning | Condition |
| --- | --- | --- |
| White `#ffffff` | Nothing recorded | `!has_capture`, or the day is in the future |
| Aqua `#8be2f8` | Raw context, not summarised | `has_capture && !has_summary` |
| Navy `#0308a3` | Summarised | `has_capture && has_summary` |
| Red `#d84a3a` | Last summarise attempt failed | job status `Failed` for that date |

Red is the reference's own fourth colour, where it marked unmovable files.
Adopting it gives failure a place to live that costs no new palette.

Days between the first recorded day and today with no file are white, in
place. A gap in the record reads as a gap.

### Field size

Columns are `floor(wellInnerWidth / 9)`, recomputed from a `ResizeObserver`
on the well. Rows are `max(20, ceil(cellCount / columns))`, where
`cellCount` covers the first recorded day through today. Any cell past
today is white.

Twenty rows is a floor so the panel holds its shape when only a few days
have been recorded. At the Overview's current width that is about 104
columns, so the field is roughly 5.7 years of canvas and will be mostly
white for a long time. This is deliberate and matches what the reference
shows for a freshly formatted disk.

Leftover horizontal pixels after the integer division are absorbed by
centring the grid in the well.

### Hover and navigation

Each cell is a `button`. Hovering shows an info box carrying the date, the
state in words, the raw size from `bytes`, the summary's `title` when there
is one, and the instruction to click. Clicking opens that day on the
Context tab.

The info box is drawn rather than a native `title` attribute. Native
tooltips arrive after a delay, are styled by macOS, and cannot hold four
lines usefully. This one follows the period: `#ffffe1` on a 1px `#000000`
border, positioned next to the cursor and flipped back inside the well when
it would overflow the right or bottom edge.

Cells for days with nothing recorded are `disabled`: there is nothing to
open.

## Data

`list_days` already returns everything the map needs and the frontend has
never called it:

```rust
pub struct DayEntry {
    pub date: NaiveDate,
    pub has_capture: bool,
    pub has_summary: bool,
    pub bytes: u64,
    pub title: Option<String>,
}
```

No new read command is needed. The map calls `list_days` on mount, after
every job completes, and when the window regains focus.

## New commands

Two, both in `src-tauri/src/lib.rs` with the queue work in
`src-tauri/src/jobs.rs`.

### `summarise_days(dates: Vec<String>) -> Result<Vec<String>, String>`

Enqueues one summarise job per date through the existing
`JobQueue::enqueue_summarise_with` with `Trigger::OnDemand`, and returns the
job ids in the same order. Rejects with the existing messages when no folder
is set or no engine is connected, matching `summarise_now`.

The caller decides the set. The map already holds the day list it renders,
so computing "every day with capture and no summary" on the frontend avoids
a second implementation of the same rule in Rust. A day summarised by
something else between render and press is harmless: it is summarised twice.

### `cancel_queued_summaries() -> usize`

The queue has no cancel today, and the obvious implementation does not
work. `drain_if_idle` takes *every* queued job into a local vector the
moment the runner is idle, then runs them serially from there. Clearing the
`VecDeque` therefore stops nothing once a batch has started, which is within
about a second of pressing Summarise.

Cancelling needs a flag the runner checks between jobs:

- `JobQueue` gains a `cancel: AtomicBool`.
- `cancel_queued_summaries` sets it, clears the `VecDeque`, marks the
  cleared jobs `Cancelled` in history, and returns how many it cleared.
- The runner's loop takes and clears the flag at the top of each iteration.
  If it was set, it marks that job and every one still to come `Cancelled`
  and stops.
- `push` clears the flag, so a cancel raised while nothing was running
  cannot kill the next batch.

The day in flight finishes. Killing the engine child process mid-write is
out of scope.

`JobStatus` gains a `Cancelled` variant. It has exactly one exhaustive
match, in `JobSummaryPayload::from`, so the compiler finds the only place
that needs updating. On the wire it is `"cancelled"`, and `DayView`'s
existing checks for `"done"` and `"failed"` ignore it, which is correct:
that view polls only the single job it started itself.

The return value is informational. The status line's "Stopped, N skipped"
counts the batch's own job ids that came back `Cancelled`, because the
queue cannot see how many the runner still held.

## Progress

The batch is a list of job ids, polled through `job_state` for each
outstanding one every 2 seconds, matching the cadence `DayView` already uses
for its own on-demand runs.

- **Progress** is `finished / total`, driven into the segmented bar.
- **Status line** shows the day being worked on, or `Ready`, or
  `Stopped, N skipped`, or the failure from the first job that failed.
- **Percentage line** shows `NN% Complete`, as the reference does.

Each time a job finishes, `list_days` is re-read so that day's cell turns
from aqua to navy while the run continues. The repaint is the point of the
view, and it falls out of the existing serial queue without new machinery.

A failed job leaves its cell red and does not stop the batch. This differs
from the scheduled path, which stops on first failure on the grounds that
every following day would fail the same way. Here the user pressed the
button and can see each result, so reporting all of them is more useful
than stopping at the first.

## Controls

A button row under the progress bar, mirroring the reference's four:

- **Legend** on the left, toggling a row of four swatches with their
  meanings between the button row and the progress bar. A toggle rather
  than the reference's modal dialog, which would be a second window for
  four words.
- **Summarise N days** on the right, disabled when nothing is pending, when
  no engine is connected, or while a batch is running. The count is in the
  label because each day is one engine run on the user's own subscription,
  and the cost should be visible before the press rather than after.
- **Stop** beside it, enabled only while a batch is running.

`MAX_BACKFILL_DAYS` continues to cap the scheduled path at seven. It does
not apply here: this button is an explicit request with the count on it.

## Wiring to the Context tab

`Main` holds the selected tab. `DayView` holds its own selected date and
already listens for an `open-day` event, which is how MCP and the tray open
a specific day.

Opening a day from the map needs the same effect on an internal route:

- `Main` gains `contextDate` state and passes it to `DayView` as an
  optional `date` prop.
- `DayView` syncs `selected` from that prop when it changes, alongside the
  existing event listener.
- The map calls `onOpenDay(date)`, which sets both the tab and the date.

## Files

| File | Change |
| --- | --- |
| `src/lib/defrag.ts` | New. Pure layout and colour helpers, plus the state hook. |
| `src/components/DefragMap.tsx` | New. The well, the grid, hover and click. |
| `src/components/DefragControls.tsx` | New. Status line, progress bar, buttons. |
| `src/components/Overview.tsx` | Status group replaced by the two above. |
| `src/components/Main.tsx` | `contextDate` state, passed to `DayView`. |
| `src/components/DayView.tsx` | Optional `date` prop synced into `selected`. |
| `src/lib/days.ts` | `DayEntry` type, mirroring the Rust struct. |
| `src/main-window.css` | Well, lattice, cell colours, progress bar. |
| `src-tauri/src/lib.rs` | Two commands, registered in the handler. |
| `src-tauri/src/jobs.rs` | `Cancelled` status, cancel flag, runner check. |

One hook and two presentational components. `useDefragState` in
`src/lib/defrag.ts` owns the day list, the failed dates, the batch and its
polling, and hands both components what they render. The alternative,
putting batch state in the controls, does not work: the map needs the failed
dates to colour cells red, so the two would have to pass state sideways.

Keeping the layout and colour rules as pure functions in the same module
means the parts worth testing need no React and no job machinery.

## Testing

- **Layout, unit.** Given a first date, a day count and a column count, the
  cell list has the right length, the right colour at each index, gaps
  white in place, and at least 20 rows.
- **Colour mapping, unit.** Each `DayEntry` shape maps to its colour,
  including a failed date overriding aqua.
- **Pending set, unit.** Days with capture and no summary, and nothing else.
- **Controls, component.** Pressing Summarise calls `summarise_days` with
  exactly the pending dates; Stop calls `cancel_queued_summaries`; the bar
  reflects finished over total.
- **Navigation, component.** Clicking a recorded cell calls `onOpenDay`
  with that date; a white cell is disabled.
- **Cancel, Rust.** `cancel_queued_summaries` empties the queue and marks
  those jobs `Cancelled`; a batch already drained into the runner stops at
  the next job boundary; a cancel raised while idle does not affect the
  next batch.

The existing `css-cascade.test.ts` guard covers the new CSS for the two
shapes it already checks.

## Out of scope

- Selecting individual days to summarise. The button acts on everything
  pending.
- Cancelling the job already in flight. The engine runs as a child process
  and killing it mid-write is a separate piece of work.
- Any change to the scheduled summarise path or to `MAX_BACKFILL_DAYS`.
- Zoom or date-range controls on the map.
