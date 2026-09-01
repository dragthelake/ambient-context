# Overview Defrag Map Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Status group on the Overview tab with a Windows 98 Disk Defragmenter map of every recorded day, plus controls to summarise every day that has raw context and no summary.

**Architecture:** Pure layout and colour helpers in `src/lib/defrag.ts`, a state hook beside them owning the day list and the batch, and two presentational components. Two new Tauri commands enqueue a batch and cancel one. Cancelling needs a flag the job runner checks between jobs, because the runner drains the whole queue at once.

**Tech Stack:** React 19, TypeScript, Vitest with jsdom and Testing Library, Tauri 2, Rust with chrono.

**Spec:** `docs/superpowers/specs/2026-09-01-defrag-overview-design.md`

## Global Constraints

- Australian English in all prose, comments and UI copy. No em-dashes (U+2014).
- Node 24.20.0, pinned in `package.json` under `volta`.
- Frontend tests live in `src/test/**/*.test.{ts,tsx}` and run with `npx vitest run`.
- No `@testing-library/jest-dom` in this repo. Assert with plain vitest: `expect((el as HTMLButtonElement).disabled).toBe(true)`, `expect(el.textContent).toContain(...)`, `toBeTruthy()`. Do not add the dependency.
- Tauri is mocked through `src/test/tauri-mock.ts`. `mockInvoke` installs a handler; an unnamed command throws, so every test names the commands it expects.
- Rust tests live in `#[cfg(test)] mod tests` at the bottom of the module they cover, and run with `cargo test` from `src-tauri`.
- CI runs, in order: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `npm run build`. All must pass.
- Palette, sampled from the reference: empty `#ffffff`, aqua `#8be2f8`, navy `#0308a3`, red `#d84a3a`, cell outline `#0a0a0a`, progress fill `#00007b`.
- Cell geometry: 7x10 fill inside a 1px outline, pitch 9x12, outlines touching so the grid reads as one lattice.
- `src/setup.css` and `src/main-window.css` share one namespace and `main-window.css` is imported first, so setup.css wins at equal specificity. New rules go in `main-window.css` with class names not used in setup.css. `src/test/css-cascade.test.ts` guards this.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `src/lib/defrag.ts` | Pure functions: cell layout, colour mapping, pending set. No React, no Tauri. |
| `src/lib/useDefragState.ts` | The hook: day list, failed dates, batch, polling. |
| `src/components/DefragMap.tsx` | The well, the lattice, the hover info box, click to open. |
| `src/components/DefragControls.tsx` | Status line, progress bar, percentage, button row, legend. |
| `src/components/Overview.tsx` | Composes the two in place of the Status group. |
| `src/components/Main.tsx` | `contextDate` state, passed to `DayView`. |
| `src/components/DayView.tsx` | Optional `date` prop synced into `selected`. |
| `src/lib/days.ts` | `DayEntry` type. |
| `src/main-window.css` | Well, lattice, cells, progress bar, info box. |
| `src-tauri/src/jobs.rs` | `Cancelled` status, cancel flag, runner check, `cancel_queued`. |
| `src-tauri/src/lib.rs` | `summarise_days`, `cancel_queued_summaries`, both registered. |

---

### Task 1: Cancellable jobs

The runner drains every queued job into a local vector as soon as it is idle, so clearing the queue stops nothing once a batch has started. Cancelling needs a flag checked between jobs.

**Files:**
- Modify: `src-tauri/src/jobs.rs`
- Modify: `src-tauri/src/lib.rs:564-578` (the one exhaustive match on `JobStatus`)

**Interfaces:**
- Consumes: nothing.
- Produces: `JobStatus::Cancelled`; `JobQueue::cancel_queued(&self) -> usize`.

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` block at the bottom of `src-tauri/src/jobs.rs`:

```rust
    #[test]
    fn cancelling_empties_the_queue_and_marks_those_jobs() {
        let queue = JobQueue::for_test();
        let a = queue.enqueue_summarise(day(2026, 8, 28));
        let b = queue.enqueue_summarise(day(2026, 8, 29));

        assert_eq!(queue.cancel_queued(), 2);

        assert_eq!(queue.find(&a.0).unwrap().status, JobStatus::Cancelled);
        assert_eq!(queue.find(&b.0).unwrap().status, JobStatus::Cancelled);
    }

    #[test]
    fn cancelling_while_idle_does_not_kill_the_next_batch() {
        let queue = JobQueue::for_test();
        queue.cancel_queued();

        // Enqueuing expresses intent to run, so it clears any stale flag.
        let next = queue.enqueue_summarise(day(2026, 8, 30));
        assert!(!queue.take_cancelled());
        assert_eq!(queue.find(&next.0).unwrap().status, JobStatus::Queued);
    }

    #[test]
    fn the_flag_is_taken_once() {
        let queue = JobQueue::for_test();
        queue.enqueue_summarise(day(2026, 8, 28));
        queue.cancel_queued();

        assert!(queue.take_cancelled());
        assert!(!queue.take_cancelled());
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cd src-tauri && cargo test cancel`
Expected: FAIL, `no method named cancel_queued`, `no variant Cancelled`.

- [ ] **Step 3: Add the status variant**

In `src-tauri/src/jobs.rs`, add to the enum:

```rust
pub enum JobStatus {
    Queued,
    Running,
    Done,
    Failed { stderr: String },
    /// Skipped because the user pressed Stop. Distinct from Done so the
    /// progress line can say how many never ran.
    Cancelled,
}
```

In `src-tauri/src/lib.rs`, add the arm the compiler now demands:

```rust
            jobs::JobStatus::Cancelled => ("cancelled", None),
```

- [ ] **Step 4: Add the flag and the methods**

In `src-tauri/src/jobs.rs`, add the field to `JobQueue`:

```rust
    /// Raised by Stop, taken by the runner between jobs. A flag rather than
    /// a queue drain because `drain_if_idle` empties the queue into a local
    /// vector the moment the runner is idle, so by the time the user can
    /// press Stop there is usually nothing left in the queue to clear.
    cancel: std::sync::atomic::AtomicBool,
```

Add the methods inside `impl JobQueue`:

```rust
    /// Empties the queue, marks what it took as cancelled, and raises the
    /// flag so the runner drops whatever it already drained. Returns how
    /// many it cleared, which is not the whole story: the runner may hold
    /// more. The caller counts its own jobs to report a total.
    pub fn cancel_queued(&self) -> usize {
        use std::sync::atomic::Ordering;
        self.cancel.store(true, Ordering::SeqCst);
        let dropped: Vec<QueuedJob> = self.queue.lock().expect("job queue").drain(..).collect();
        for job in &dropped {
            self.record(&job.id, JobStatus::Cancelled);
        }
        dropped.len()
    }

    /// Reads and clears the flag. The runner calls this before each job, so
    /// one press of Stop cancels one batch and not the next.
    pub fn take_cancelled(&self) -> bool {
        use std::sync::atomic::Ordering;
        self.cancel.swap(false, Ordering::SeqCst)
    }
```

In `push`, clear the flag so a stale cancel cannot kill a fresh batch. Add as the first line of the method body:

```rust
        self.cancel
            .store(false, std::sync::atomic::Ordering::SeqCst);
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test cancel`
Expected: PASS, 3 tests.

- [ ] **Step 6: Make the runner honour the flag**

In `src-tauri/src/jobs.rs`, replace the body of the `for job in queued` loop's opening so it checks first. The loop currently starts:

```rust
    for job in queued {
        let date = job.date;
        app.state::<JobQueue>().record(&job.id, JobStatus::Running);
```

Make it:

```rust
    let mut cancelled = false;
    for job in queued {
        // Taken once per job, so Stop drops everything still to come in
        // this batch without touching the next one.
        if cancelled || app.state::<JobQueue>().take_cancelled() {
            cancelled = true;
            app.state::<JobQueue>().record(&job.id, JobStatus::Cancelled);
            continue;
        }
        let date = job.date;
        app.state::<JobQueue>().record(&job.id, JobStatus::Running);
```

- [ ] **Step 7: Run the whole suite and the CI gates**

Run: `cd src-tauri && cargo test && cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/jobs.rs src-tauri/src/lib.rs
git commit -m "feat: let a queued batch of summaries be cancelled

The runner drains every queued job into a local vector as soon as it is
idle, so clearing the queue stops nothing once a batch has started.
Cancelling is a flag the runner takes between jobs instead, and the jobs
it drops get their own status rather than being reported as done."
```

---

### Task 2: The batch commands

**Files:**
- Modify: `src-tauri/src/lib.rs` (beside `summarise_now`, around line 585; and the `invoke_handler` list around line 1427)

**Interfaces:**
- Consumes: `JobQueue::enqueue_summarise_with`, `JobQueue::cancel_queued` from Task 1.
- Produces: two Tauri commands.
  - `summarise_days(dates: Vec<String>) -> Result<Vec<String>, String>` returns job ids in the order given.
  - `cancel_queued_summaries() -> usize`.

- [ ] **Step 1: Write the commands**

Add after `summarise_now` in `src-tauri/src/lib.rs`:

```rust
/// Enqueues one summarise per date and hands back the job ids in the same
/// order, so the window can poll the batch it just started.
///
/// The caller picks the set. The Overview map already holds the day list it
/// draws, so deciding "has capture, has no summary" there avoids a second
/// implementation of the same rule. A day summarised by something else in
/// between is simply summarised twice.
#[tauri::command]
fn summarise_days(app: tauri::AppHandle, dates: Vec<String>) -> Result<Vec<String>, String> {
    let config = settings::load(&app);
    if config.folder.is_none() {
        return Err("no capture folder is set".to_string());
    }
    if config.engine.is_none() {
        return Err("no engine is connected".to_string());
    }
    let queue = app.state::<jobs::JobQueue>();
    let mut ids = Vec::with_capacity(dates.len());
    for date in &dates {
        let parsed = parse_date(date)?;
        ids.push(
            queue
                .enqueue_summarise_with(parsed, ledger::Trigger::OnDemand)
                .to_string(),
        );
    }
    Ok(ids)
}

/// Stops a batch. The day already in flight finishes; everything after it is
/// dropped. Returns how many were still queued, which is informational: the
/// runner may hold more, so the window counts its own cancelled ids.
#[tauri::command]
fn cancel_queued_summaries(app: tauri::AppHandle) -> usize {
    app.state::<jobs::JobQueue>().cancel_queued()
}
```

- [ ] **Step 2: Register them**

In the `invoke_handler` list in `src-tauri/src/lib.rs`, add after `summarise_now,`:

```rust
            summarise_days,
            cancel_queued_summaries,
```

- [ ] **Step 3: Verify it compiles and the gates pass**

Run: `cd src-tauri && cargo test && cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: add commands to run and stop a batch of summaries

summarise_days enqueues one job per date and returns the ids so the window
can poll the batch. The caller picks the set, because the Overview map
already holds the day list and a second copy of the rule in Rust would only
drift."
```

---

### Task 3: Layout and colour, as pure functions

**Files:**
- Create: `src/lib/defrag.ts`
- Create: `src/test/defrag.test.ts`
- Modify: `src/lib/days.ts` (add the `DayEntry` type)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `type DayEntry = { date: string; has_capture: boolean; has_summary: boolean; bytes: number; title: string | null }` in `src/lib/days.ts`
  - `type CellState = "empty" | "raw" | "summarised" | "failed"`
  - `type Cell = { date: string; state: CellState; entry: DayEntry | null }`
  - `buildCells(days: DayEntry[], today: string, columns: number, failed: Set<string>): Cell[]`
  - `pendingDates(days: DayEntry[]): string[]`
  - `MIN_ROWS = 20`, `CELL_W = 9`, `CELL_H = 12`

- [ ] **Step 1: Write the failing tests**

Create `src/test/defrag.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { buildCells, MIN_ROWS, pendingDates } from "../lib/defrag";
import type { DayEntry } from "../lib/days";

function entry(date: string, over: Partial<DayEntry> = {}): DayEntry {
  return {
    date,
    has_capture: true,
    has_summary: false,
    bytes: 100,
    title: null,
    ...over,
  };
}

describe("buildCells", () => {
  it("fills at least twenty rows even with one day", () => {
    const cells = buildCells([entry("2026-09-01")], "2026-09-01", 10, new Set());
    expect(cells).toHaveLength(MIN_ROWS * 10);
  });

  it("starts at the first recorded day and runs to today", () => {
    const cells = buildCells([entry("2026-08-30")], "2026-09-01", 10, new Set());
    expect(cells[0].date).toBe("2026-08-30");
    expect(cells[1].date).toBe("2026-08-31");
    expect(cells[2].date).toBe("2026-09-01");
  });

  it("colours by capture and summary, and leaves gaps empty in place", () => {
    const days = [
      entry("2026-08-30"),
      entry("2026-09-01", { has_summary: true }),
    ];
    const cells = buildCells(days, "2026-09-01", 10, new Set());
    expect(cells[0].state).toBe("raw");
    expect(cells[1].state).toBe("empty");
    expect(cells[2].state).toBe("summarised");
  });

  it("marks a failed date red over its raw colour", () => {
    const cells = buildCells(
      [entry("2026-09-01")],
      "2026-09-01",
      10,
      new Set(["2026-09-01"]),
    );
    expect(cells[0].state).toBe("failed");
  });

  it("leaves every cell past today empty", () => {
    const cells = buildCells([entry("2026-09-01")], "2026-09-01", 10, new Set());
    expect(cells[1].state).toBe("empty");
    expect(cells[1].entry).toBeNull();
  });

  it("grows past twenty rows when there are more days than fit", () => {
    const cells = buildCells([entry("2024-01-01")], "2026-09-01", 10, new Set());
    expect(cells.length).toBeGreaterThan(MIN_ROWS * 10);
    expect(cells.length % 10).toBe(0);
  });

  it("returns nothing when no day has been recorded", () => {
    expect(buildCells([], "2026-09-01", 10, new Set())).toEqual([]);
  });
});

describe("pendingDates", () => {
  it("is days with capture and no summary, oldest first", () => {
    const days = [
      entry("2026-09-01"),
      entry("2026-08-30", { has_summary: true }),
      entry("2026-08-31"),
      entry("2026-08-29", { has_capture: false }),
    ];
    expect(pendingDates(days)).toEqual(["2026-08-31", "2026-09-01"]);
  });
});
```

- [ ] **Step 2: Run them to verify they fail**

Run: `npx vitest run src/test/defrag.test.ts`
Expected: FAIL, cannot resolve `../lib/defrag`.

- [ ] **Step 3: Add the DayEntry type**

Append to `src/lib/days.ts`:

```ts
/// One day as `list_days` reports it. Mirrors `days::DayEntry` in Rust:
/// `date` arrives as the `YYYY-MM-DD` string chrono serialises a NaiveDate
/// to, not as a Date.
export type DayEntry = {
  date: string;
  has_capture: boolean;
  has_summary: boolean;
  bytes: number;
  title: string | null;
};
```

- [ ] **Step 4: Write the implementation**

Create `src/lib/defrag.ts`:

```ts
import type { DayEntry } from "./days";

/// Measured from the reference screenshot: a 7x10 fill inside a 1px
/// outline. Adjacent outlines touch rather than leaving a gutter, which is
/// what makes the grid read as one black lattice rather than loose tiles.
export const CELL_W = 9;
export const CELL_H = 12;

/// A floor, not a target. It keeps the panel's shape when only a few days
/// have been recorded. At the Overview's width that is several years of
/// canvas, mostly white, which is what the reference shows for a disk with
/// little on it.
export const MIN_ROWS = 20;

export type CellState = "empty" | "raw" | "summarised" | "failed";

export type Cell = {
  date: string;
  state: CellState;
  entry: DayEntry | null;
};

function iso(date: Date): string {
  const month = String(date.getUTCMonth() + 1).padStart(2, "0");
  const day = String(date.getUTCDate()).padStart(2, "0");
  return `${date.getUTCFullYear()}-${month}-${day}`;
}

/// Dates are handled in UTC throughout. They are calendar days, never
/// instants, and parsing `YYYY-MM-DD` as local time shifts the whole map by
/// one for anyone west of Greenwich.
function parse(value: string): Date {
  return new Date(`${value}T00:00:00Z`);
}

function addDays(date: Date, days: number): Date {
  const next = new Date(date);
  next.setUTCDate(next.getUTCDate() + days);
  return next;
}

/// Every day from the first recorded one to today, wrapped into `columns`
/// and padded with empty cells to at least `MIN_ROWS`. Days with no file
/// stay in place as empty cells, so a gap in the record reads as a gap.
export function buildCells(
  days: DayEntry[],
  today: string,
  columns: number,
  failed: Set<string>,
): Cell[] {
  if (days.length === 0 || columns <= 0) return [];

  const byDate = new Map(days.map((day) => [day.date, day]));
  const first = days.reduce(
    (earliest, day) => (day.date < earliest ? day.date : earliest),
    days[0].date,
  );

  const cells: Cell[] = [];
  for (let at = parse(first); iso(at) <= today; at = addDays(at, 1)) {
    const date = iso(at);
    const entry = byDate.get(date) ?? null;
    cells.push({ date, entry, state: stateOf(entry, failed.has(date)) });
  }

  const rows = Math.max(MIN_ROWS, Math.ceil(cells.length / columns));
  const total = rows * columns;
  for (let n = cells.length; n < total; n += 1) {
    cells.push({ date: "", state: "empty", entry: null });
  }
  return cells;
}

function stateOf(entry: DayEntry | null, hasFailed: boolean): CellState {
  if (!entry || !entry.has_capture) return "empty";
  if (hasFailed) return "failed";
  return entry.has_summary ? "summarised" : "raw";
}

/// Days holding raw context with no summary, oldest first, which is the
/// order the runner works through them.
export function pendingDates(days: DayEntry[]): string[] {
  return days
    .filter((day) => day.has_capture && !day.has_summary)
    .map((day) => day.date)
    .sort();
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `npx vitest run src/test/defrag.test.ts`
Expected: PASS, 8 tests.

- [ ] **Step 6: Commit**

```bash
git add src/lib/defrag.ts src/lib/days.ts src/test/defrag.test.ts
git commit -m "feat: lay out the defrag map as pure functions

Cell geometry and the minimum twenty rows are measured from the reference.
Dates are handled in UTC because they are calendar days, and parsing them
as local time shifts the whole map by one west of Greenwich."
```

---

### Task 4: The map component

**Files:**
- Create: `src/components/DefragMap.tsx`
- Create: `src/test/DefragMap.test.tsx`
- Modify: `src/main-window.css` (append at the end of the file)

**Interfaces:**
- Consumes: `buildCells`, `Cell`, `CELL_W`, `CELL_H` from Task 3.
- Produces: `DefragMap({ days, failed, today, onOpenDay }: { days: DayEntry[]; failed: Set<string>; today: string; onOpenDay: (date: string) => void })`.

- [ ] **Step 1: Write the failing tests**

Create `src/test/DefragMap.test.tsx`:

```tsx
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DefragMap } from "../components/DefragMap";
import type { DayEntry } from "../lib/days";

afterEach(cleanup);

function entry(date: string, over: Partial<DayEntry> = {}): DayEntry {
  return { date, has_capture: true, has_summary: false, bytes: 2048, title: null, ...over };
}

describe("DefragMap", () => {
  it("opens the day when a recorded cell is clicked", () => {
    const onOpenDay = vi.fn();
    render(
      <DefragMap
        days={[entry("2026-09-01")]}
        failed={new Set()}
        today="2026-09-01"
        onOpenDay={onOpenDay}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /1 September 2026/ }));
    expect(onOpenDay).toHaveBeenCalledWith("2026-09-01");
  });

  it("disables cells with nothing recorded", () => {
    render(
      <DefragMap
        days={[entry("2026-08-30")]}
        failed={new Set()}
        today="2026-09-01"
        onOpenDay={vi.fn()}
      />,
    );
    const gap = screen.getByRole("button", { name: /31 August 2026/ });
    expect((gap as HTMLButtonElement).disabled).toBe(true);
  });

  it("shows the info box on hover and hides it on leave", () => {
    render(
      <DefragMap
        days={[entry("2026-09-01", { title: "Shipped the map" })]}
        failed={new Set()}
        today="2026-09-01"
        onOpenDay={vi.fn()}
      />,
    );
    const cell = screen.getByRole("button", { name: /1 September 2026/ });
    expect(screen.queryByRole("tooltip")).toBeNull();
    fireEvent.mouseEnter(cell);
    expect(screen.getByRole("tooltip").textContent).toContain("Shipped the map");
    expect(screen.getByRole("tooltip").textContent).toContain("2.0 KB");
    fireEvent.mouseLeave(cell);
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("renders nothing but the well when no day has been recorded", () => {
    render(
      <DefragMap days={[]} failed={new Set()} today="2026-09-01" onOpenDay={vi.fn()} />,
    );
    expect(screen.queryAllByRole("button")).toHaveLength(0);
  });
});
```

- [ ] **Step 2: Run them to verify they fail**

Run: `npx vitest run src/test/DefragMap.test.tsx`
Expected: FAIL, cannot resolve `../components/DefragMap`.

- [ ] **Step 3: Write the component**

Create `src/components/DefragMap.tsx`:

```tsx
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { buildCells, CELL_W, type Cell } from "../lib/defrag";
import type { DayEntry } from "../lib/days";

const STATE_WORDS: Record<Cell["state"], string> = {
  empty: "Nothing recorded",
  raw: "Raw context, not summarised",
  summarised: "Summarised",
  failed: "Last summarise failed",
};

function longDate(iso: string): string {
  return new Date(`${iso}T00:00:00Z`).toLocaleDateString("en-AU", {
    day: "numeric",
    month: "long",
    year: "numeric",
    timeZone: "UTC",
  });
}

function size(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/// One cell per day in a Windows 98 Disk Defragmenter field. Columns come
/// from the well's own width rather than a constant, so the map reflows
/// with the window.
export function DefragMap({
  days,
  failed,
  today,
  onOpenDay,
}: {
  days: DayEntry[];
  failed: Set<string>;
  today: string;
  onOpenDay: (date: string) => void;
}) {
  const well = useRef<HTMLDivElement>(null);
  const [columns, setColumns] = useState(1);
  const [hovered, setHovered] = useState<Cell | null>(null);

  // Measured, not assumed: the pane is resizable and the map is the only
  // thing on the tab whose shape depends on its own width.
  useLayoutEffect(() => {
    const node = well.current;
    if (!node) return;
    const measure = () =>
      setColumns(Math.max(1, Math.floor(node.clientWidth / CELL_W)));
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  const cells = buildCells(days, today, columns, failed);

  return (
    <div className="defrag-well" ref={well}>
      <div
        className="defrag-grid"
        style={{ gridTemplateColumns: `repeat(${columns}, ${CELL_W}px)` }}
      >
        {cells.map((cell, index) => (
          <button
            key={cell.date || `pad-${index}`}
            type="button"
            className={`defrag-cell is-${cell.state}`}
            disabled={cell.entry === null}
            aria-label={cell.date ? longDate(cell.date) : undefined}
            onMouseEnter={() => setHovered(cell)}
            onMouseLeave={() => setHovered(null)}
            onFocus={() => setHovered(cell)}
            onBlur={() => setHovered(null)}
            onClick={() => cell.entry && onOpenDay(cell.date)}
          />
        ))}
      </div>
      {hovered?.entry ? (
        <div className="defrag-info" role="tooltip">
          <strong>{longDate(hovered.date)}</strong>
          <span>{STATE_WORDS[hovered.state]}</span>
          <span>{size(hovered.entry.bytes)}</span>
          {hovered.entry.title ? <span>{hovered.entry.title}</span> : null}
          <span className="defrag-info-hint">Click to open in Context</span>
        </div>
      ) : null}
    </div>
  );
}
```

- [ ] **Step 4: Add the CSS**

Append to `src/main-window.css`:

```css
/* The defrag map
   Geometry and palette are measured from the reference screenshot in
   docs/reference, not estimated: a 7x10 fill inside a 1px outline, so the
   pitch is 9x12. Adjacent outlines touch, which is what makes the field
   read as one black lattice rather than a scatter of tiles. */

.defrag-well {
  position: relative;
  background: var(--well);
  box-shadow: var(--bevel-in);
  /* Holds the grid inside the bevel, the same reason .tabpane carries it. */
  padding: 2px;
  overflow: hidden;
}

.defrag-grid {
  display: grid;
  /* No gap: the cells' own outlines meet to form the lattice. */
  gap: 0;
  justify-content: center;
}

.defrag-cell {
  appearance: none;
  width: 9px;
  height: 12px;
  min-width: 0;
  padding: 0;
  margin: 0;
  border: 1px solid #0a0a0a;
  box-shadow: none;
  background: #ffffff;
}

.defrag-cell.is-raw {
  background: #8be2f8;
}

.defrag-cell.is-summarised {
  background: #0308a3;
}

.defrag-cell.is-failed {
  background: #d84a3a;
}

.defrag-cell:disabled {
  /* Still drawn, just not a target. The default disabled treatment would
     grey the fill and lose the colour the map exists to show. */
  color: inherit;
}

/* The period's own info box, not a native title attribute: those arrive
   after a delay, are styled by macOS, and cannot hold four lines. */
.defrag-info {
  position: absolute;
  left: 6px;
  bottom: 6px;
  display: flex;
  flex-direction: column;
  background: #ffffe1;
  border: 1px solid var(--chrome-darker);
  padding: 3px 6px;
  font-size: 11px;
  line-height: 1.35;
  pointer-events: none;
}

.defrag-info-hint {
  color: var(--chrome-dark);
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `npx vitest run src/test/DefragMap.test.tsx`
Expected: PASS, 4 tests.

Note: jsdom reports `clientWidth` as 0, so `columns` falls back to 1 and the cells render in a single column. The tests assert on roles and labels, never on layout, so this is fine.

- [ ] **Step 6: Check the cascade guard still passes**

Run: `npx vitest run src/test/css-cascade.test.ts`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/components/DefragMap.tsx src/test/DefragMap.test.tsx src/main-window.css
git commit -m "feat: draw the defrag map of recorded days

Columns come from a ResizeObserver on the well rather than a constant, so
the field reflows with the window. The hover panel is drawn rather than a
native title attribute, which arrives late, is styled by macOS and cannot
hold four lines."
```

---

### Task 5: Batch state and polling

**Files:**
- Create: `src/lib/useDefragState.ts`
- Create: `src/test/useDefragState.test.tsx`

**Interfaces:**
- Consumes: `pendingDates` from Task 3.
- Produces: `useDefragState()` returning
  `{ days: DayEntry[]; failed: Set<string>; today: string; pending: string[]; running: boolean; finished: number; total: number; status: string; start: () => Promise<void>; stop: () => Promise<void>; reload: () => Promise<void> }`.

- [ ] **Step 1: Write the failing tests**

Create `src/test/useDefragState.test.tsx`:

```tsx
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { callsOf, mockInvoke } from "./tauri-mock";
import { useDefragState } from "../lib/useDefragState";

vi.mock("@tauri-apps/api/core", async () => {
  const mock = await import("./tauri-mock");
  return { invoke: mock.invoke };
});

afterEach(cleanup);
beforeEach(() => vi.useRealTimers());

function Probe() {
  const state = useDefragState();
  return (
    <div>
      <span data-testid="pending">{state.pending.join(",")}</span>
      <span data-testid="progress">{`${state.finished}/${state.total}`}</span>
      <span data-testid="status">{state.status}</span>
      <button type="button" onClick={() => void state.start()}>start</button>
      <button type="button" onClick={() => void state.stop()}>stop</button>
    </div>
  );
}

describe("useDefragState", () => {
  it("reads the day list and works out what is pending", async () => {
    mockInvoke((command) => {
      if (command === "list_days") {
        return [
          { date: "2026-09-01", has_capture: true, has_summary: false, bytes: 1, title: null },
          { date: "2026-08-31", has_capture: true, has_summary: true, bytes: 1, title: null },
        ];
      }
      throw new Error(`unexpected ${command}`);
    });
    render(<Probe />);
    await waitFor(() =>
      expect(screen.getByTestId("pending").textContent).toBe("2026-09-01"),
    );
  });

  it("starts a batch with exactly the pending days", async () => {
    mockInvoke((command) => {
      switch (command) {
        case "list_days":
          return [
            { date: "2026-09-01", has_capture: true, has_summary: false, bytes: 1, title: null },
            { date: "2026-08-31", has_capture: true, has_summary: true, bytes: 1, title: null },
          ];
        case "summarise_days":
          return ["job-0"];
        case "job_state":
          return { id: "job-0", date: "2026-09-01", status: "queued", stderr: null };
        default:
          throw new Error(`unexpected ${command}`);
      }
    });
    render(<Probe />);
    await waitFor(() =>
      expect(screen.getByTestId("pending").textContent).toBe("2026-09-01"),
    );
    await act(async () => {
      screen.getByText("start").click();
    });
    expect(callsOf("summarise_days")[0].args).toEqual({ dates: ["2026-09-01"] });
    expect(screen.getByTestId("progress").textContent).toBe("0/1");
  });

  it("stops a batch through cancel_queued_summaries", async () => {
    mockInvoke((command) => {
      switch (command) {
        case "list_days":
          return [{ date: "2026-09-01", has_capture: true, has_summary: false, bytes: 1, title: null }];
        case "summarise_days":
          return ["job-0"];
        case "job_state":
          return { id: "job-0", date: "2026-09-01", status: "queued", stderr: null };
        case "cancel_queued_summaries":
          return 1;
        default:
          throw new Error(`unexpected ${command}`);
      }
    });
    render(<Probe />);
    await waitFor(() =>
      expect(screen.getByTestId("pending").textContent).toBe("2026-09-01"),
    );
    await act(async () => {
      screen.getByText("start").click();
    });
    await act(async () => {
      screen.getByText("stop").click();
    });
    expect(callsOf("cancel_queued_summaries")).toHaveLength(1);
  });
});
```

- [ ] **Step 2: Run them to verify they fail**

Run: `npx vitest run src/test/useDefragState.test.tsx`
Expected: FAIL, cannot resolve `../lib/useDefragState`.

- [ ] **Step 3: Write the hook**

Create `src/lib/useDefragState.ts`:

```ts
import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { pendingDates } from "./defrag";
import type { DayEntry } from "./days";

type JobState = {
  id: string;
  date: string;
  status: "queued" | "running" | "done" | "failed" | "cancelled";
  stderr: string | null;
};

/// The same cadence DayView uses for its own on-demand run.
const POLL_MS = 2000;

function todayIso(): string {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
}

/// Owns the day list, the failed dates and the batch. Both the map and the
/// controls are handed what they draw, because the map needs the failed set
/// to colour cells and the controls need the same batch to fill the bar:
/// splitting the state between them would mean passing it sideways.
export function useDefragState() {
  const [days, setDays] = useState<DayEntry[]>([]);
  const [failed, setFailed] = useState<Set<string>>(new Set());
  const [jobs, setJobs] = useState<string[]>([]);
  const [done, setDone] = useState<Set<string>>(new Set());
  const [cancelled, setCancelled] = useState(0);
  const [status, setStatus] = useState("Ready");
  const running = jobs.length > 0 && done.size < jobs.length;
  const today = useRef(todayIso());

  const reload = useCallback(async () => {
    setDays(await invoke<DayEntry[]>("list_days"));
  }, []);

  useEffect(() => {
    void reload();
    // Capture keeps writing while the window is in the background, so the
    // map is stale by the time it comes back.
    const refresh = () => void reload();
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [reload]);

  const start = useCallback(async () => {
    const dates = pendingDates(days);
    if (dates.length === 0) return;
    setDone(new Set());
    setCancelled(0);
    setFailed(new Set());
    try {
      const ids = await invoke<string[]>("summarise_days", { dates });
      setJobs(ids);
      setStatus(`Summarising ${dates.length} days`);
    } catch (error) {
      setStatus(String(error));
    }
  }, [days]);

  const stop = useCallback(async () => {
    await invoke<number>("cancel_queued_summaries");
    setStatus("Stopping");
  }, []);

  // Poll only what is outstanding. A finished job never changes again, so
  // asking after it is a request per job per tick for no new information.
  useEffect(() => {
    if (!running) return;
    let live = true;
    const id = setInterval(() => {
      void (async () => {
        const outstanding = jobs.filter((job) => !done.has(job));
        for (const job of outstanding) {
          const state = await invoke<JobState | null>("job_state", { jobId: job });
          if (!live || !state) continue;
          if (state.status === "done") {
            setDone((current) => new Set(current).add(job));
            await reload();
          }
          if (state.status === "failed") {
            setDone((current) => new Set(current).add(job));
            setFailed((current) => new Set(current).add(state.date));
            setStatus(state.stderr ?? `${state.date} failed`);
          }
          if (state.status === "cancelled") {
            setDone((current) => new Set(current).add(job));
            setCancelled((n) => n + 1);
          }
          if (state.status === "running") {
            setStatus(`Summarising ${state.date}`);
          }
        }
      })();
    }, POLL_MS);
    return () => {
      live = false;
      clearInterval(id);
    };
  }, [running, jobs, done, reload]);

  // The batch has finished. Say how it went once, not every tick.
  useEffect(() => {
    if (jobs.length === 0 || done.size < jobs.length) return;
    setStatus(cancelled > 0 ? `Stopped, ${cancelled} skipped` : "Ready");
  }, [jobs, done, cancelled]);

  return {
    days,
    failed,
    today: today.current,
    pending: pendingDates(days),
    running,
    finished: done.size,
    total: jobs.length,
    status,
    start,
    stop,
    reload,
  };
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npx vitest run src/test/useDefragState.test.tsx`
Expected: PASS, 3 tests.

- [ ] **Step 5: Commit**

```bash
git add src/lib/useDefragState.ts src/test/useDefragState.test.tsx
git commit -m "feat: hold the defrag map's day list and batch in one hook

One owner rather than two, because the map needs the failed dates to colour
cells and the controls need the same batch to fill the bar. Only
outstanding jobs are polled: a finished one never changes again."
```

---

### Task 6: The controls

**Files:**
- Create: `src/components/DefragControls.tsx`
- Create: `src/test/DefragControls.test.tsx`
- Modify: `src/main-window.css` (append)

**Interfaces:**
- Consumes: the return type of `useDefragState` from Task 5.
- Produces: `DefragControls({ pending, running, finished, total, status, hasEngine, onStart, onStop })`.

- [ ] **Step 1: Write the failing tests**

Create `src/test/DefragControls.test.tsx`:

```tsx
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DefragControls } from "../components/DefragControls";

afterEach(cleanup);

const base = {
  pending: ["2026-09-01"],
  running: false,
  finished: 0,
  total: 0,
  status: "Ready",
  hasEngine: true,
  onStart: vi.fn(),
  onStop: vi.fn(),
};

describe("DefragControls", () => {
  it("puts the count in the button so the cost is visible before pressing", () => {
    render(<DefragControls {...base} pending={["2026-09-01", "2026-08-31"]} />);
    const go = screen.getByRole("button", { name: "Summarise 2 days" });
    expect((go as HTMLButtonElement).disabled).toBe(false);
  });

  it("says one day rather than 1 days", () => {
    render(<DefragControls {...base} />);
    const go = screen.getByRole("button", { name: "Summarise 1 day" });
    expect((go as HTMLButtonElement).disabled).toBe(false);
  });

  it("disables Summarise with no engine connected", () => {
    render(<DefragControls {...base} hasEngine={false} />);
    const go = screen.getByRole("button", { name: /Summarise/ });
    expect((go as HTMLButtonElement).disabled).toBe(true);
  });

  it("disables Summarise when nothing is pending", () => {
    render(<DefragControls {...base} pending={[]} />);
    const go = screen.getByRole("button", { name: /Summarise/ });
    expect((go as HTMLButtonElement).disabled).toBe(true);
  });

  it("enables Stop only while running", () => {
    const { rerender } = render(<DefragControls {...base} />);
    const stop = () => screen.getByRole("button", { name: "Stop" }) as HTMLButtonElement;
    expect(stop().disabled).toBe(true);
    rerender(<DefragControls {...base} running total={2} finished={1} />);
    expect(stop().disabled).toBe(false);
  });

  it("reports progress as the reference does", () => {
    render(<DefragControls {...base} running total={4} finished={1} />);
    expect(screen.getByText("25% Complete")).toBeTruthy();
    expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("25");
  });

  it("shows nothing complete before a run starts", () => {
    render(<DefragControls {...base} />);
    expect(screen.getByText("0% Complete")).toBeTruthy();
  });

  it("toggles the legend", () => {
    render(<DefragControls {...base} />);
    expect(screen.queryByText("Raw context")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Legend" }));
    expect(screen.getByText("Raw context")).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run them to verify they fail**

Run: `npx vitest run src/test/DefragControls.test.tsx`
Expected: FAIL, cannot resolve `../components/DefragControls`.

- [ ] **Step 3: Write the component**

Create `src/components/DefragControls.tsx`:

```tsx
import { useState } from "react";

const LEGEND: { state: string; label: string }[] = [
  { state: "empty", label: "Nothing recorded" },
  { state: "raw", label: "Raw context" },
  { state: "summarised", label: "Summarised" },
  { state: "failed", label: "Failed" },
];

/// The reference's lower half: a status line, a segmented bar in a sunken
/// trough, a percentage, and a button row with two at each end.
export function DefragControls({
  pending,
  running,
  finished,
  total,
  status,
  hasEngine,
  onStart,
  onStop,
}: {
  pending: string[];
  running: boolean;
  finished: number;
  total: number;
  status: string;
  hasEngine: boolean;
  onStart: () => void;
  onStop: () => void;
}) {
  const [legend, setLegend] = useState(false);
  const percent = total === 0 ? 0 : Math.round((finished / total) * 100);
  const days = pending.length === 1 ? "1 day" : `${pending.length} days`;

  return (
    <div className="defrag-controls">
      <p className="defrag-status">{status}</p>

      {/* Segmented, as the period's bars were: whole blocks appear rather
          than a bar sliding, so progress is countable at a glance. */}
      <div
        className="defrag-bar"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={percent}
      >
        <div className="defrag-bar-fill" style={{ width: `${percent}%` }} />
      </div>

      <p className="defrag-percent">{percent}% Complete</p>

      {legend ? (
        <ul className="defrag-legend">
          {LEGEND.map((item) => (
            <li key={item.state}>
              <span className={`defrag-swatch is-${item.state}`} aria-hidden="true" />
              {item.label}
            </li>
          ))}
        </ul>
      ) : null}

      <div className="defrag-buttons">
        <button type="button" onClick={() => setLegend((on) => !on)}>
          Legend
        </button>
        <span className="defrag-spacer" />
        <button
          type="button"
          disabled={running || pending.length === 0 || !hasEngine}
          title={hasEngine ? undefined : "Connect an engine in Settings to use this."}
          onClick={onStart}
        >
          {`Summarise ${days}`}
        </button>
        <button type="button" disabled={!running} onClick={onStop}>
          Stop
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Add the CSS**

Append to `src/main-window.css`:

```css
.defrag-controls {
  margin-top: 6px;
}

.defrag-status,
.defrag-percent {
  margin: 4px 0;
  font-size: 12px;
}

/* The trough is sunken and the fill is the reference's navy. It carries
   the same 2px padding the panes do, for the same reason: an inset shadow
   paints beneath its descendants, so without it the fill covers the bevel. */
.defrag-bar {
  height: 20px;
  background: var(--chrome);
  box-shadow: var(--bevel-in);
  padding: 2px;
  overflow: hidden;
}

.defrag-bar-fill {
  height: 100%;
  /* Blocks with gaps, not a solid bar: repeating-linear-gradient gives the
     period's segmented fill without an element per segment. */
  background: repeating-linear-gradient(
    90deg,
    #00007b 0 10px,
    transparent 10px 12px
  );
  transition: width 120ms linear;
}

.defrag-legend {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 14px;
  margin: 6px 0 0;
  padding: 0;
  list-style: none;
  font-size: 12px;
}

.defrag-legend li {
  display: flex;
  align-items: center;
  gap: 5px;
}

.defrag-swatch {
  width: 9px;
  height: 12px;
  border: 1px solid #0a0a0a;
  background: #ffffff;
}

.defrag-swatch.is-raw {
  background: #8be2f8;
}

.defrag-swatch.is-summarised {
  background: #0308a3;
}

.defrag-swatch.is-failed {
  background: #d84a3a;
}

.defrag-buttons {
  display: flex;
  gap: 6px;
  margin-top: 8px;
}

.defrag-spacer {
  flex: 1;
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `npx vitest run src/test/DefragControls.test.tsx`
Expected: PASS, 8 tests.

- [ ] **Step 6: Commit**

```bash
git add src/components/DefragControls.tsx src/test/DefragControls.test.tsx src/main-window.css
git commit -m "feat: add the defrag status line, progress bar and buttons

The day count is in the Summarise label because each day is one engine run
on the user's own subscription, and the cost should be visible before the
press rather than after."
```

---

### Task 7: Wire it into Overview and the Context tab

**Files:**
- Modify: `src/components/Overview.tsx`
- Modify: `src/components/Main.tsx`
- Modify: `src/components/DayView.tsx`
- Modify: `src/test/Main.test.tsx`

**Interfaces:**
- Consumes: `DefragMap` (Task 4), `useDefragState` (Task 5), `DefragControls` (Task 6).
- Produces: `Overview({ status, onOpenDay }: { status: AppStatus; onOpenDay: (date: string) => void })`; `DayView({ date }: { date?: string })`.

- [ ] **Step 1: Write the failing test**

Add to `src/test/Main.test.tsx`, inside its existing `describe`:

```tsx
  it("opens a day on the Context tab when a map cell is clicked", async () => {
    render(<Main />);
    await waitFor(() =>
      expect(screen.getAllByRole("button", { name: /September 2026/ }).length).toBeGreaterThan(0),
    );
    fireEvent.click(screen.getAllByRole("button", { name: /September 2026/ })[0]);
    await waitFor(() =>
      expect(
        screen.getByRole("tab", { name: "Context" }).getAttribute("aria-selected"),
      ).toBe("true"),
    );
  });
```

Add `list_days`, `summarise_days` and `cancel_queued_summaries` to that file's existing invoke handler, returning `[]`, `[]` and `0`.

- [ ] **Step 2: Run it to verify it fails**

Run: `npx vitest run src/test/Main.test.tsx`
Expected: FAIL, no button matching the date.

- [ ] **Step 3: Replace the Status group in Overview**

Rewrite `src/components/Overview.tsx` entirely. The Status group goes; the
eye and the "Finish setup" escape hatch stay.

```tsx
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { DefragControls } from "./DefragControls";
import { DefragMap } from "./DefragMap";
import { EyePanel } from "./EyePanel";
import { useDefragState } from "../lib/useDefragState";
import type { AppStatus } from "../lib/status";
import type { Settings } from "../lib/days";

/// The eye, then the record. The Status group that used to sit here said
/// what the status bar along the bottom of the window already says.
export function Overview({
  status,
  onOpenDay,
}: {
  status: AppStatus;
  onOpenDay: (date: string) => void;
}) {
  const { capture, setCapture, ready } = status;
  const defrag = useDefragState();
  const [hasEngine, setHasEngine] = useState(false);

  useEffect(() => {
    void invoke<Settings>("get_settings").then((saved) =>
      setHasEngine(saved.engine !== null),
    );
  }, []);

  return (
    <div className="overview">
      <EyePanel capture={capture} ready={ready} onCapture={setCapture} />

      <fieldset>
        <legend>Record</legend>
        <DefragMap
          days={defrag.days}
          failed={defrag.failed}
          today={defrag.today}
          onOpenDay={onOpenDay}
        />
        <DefragControls
          pending={defrag.pending}
          running={defrag.running}
          finished={defrag.finished}
          total={defrag.total}
          status={defrag.status}
          hasEngine={hasEngine}
          onStart={() => void defrag.start()}
          onStop={() => void defrag.stop()}
        />
        {ready ? null : (
          <div className="button-row">
            <button type="button" onClick={() => void invoke("open_setup")}>
              Finish setup…
            </button>
          </div>
        )}
      </fieldset>
    </div>
  );
}
```

- [ ] **Step 4: Hold the date in Main**

In `src/components/Main.tsx`, add beside the existing tab state:

```tsx
  const [contextDate, setContextDate] = useState<string | null>(null);
```

Pass it down, and give Overview the callback that moves both:

```tsx
  const openDay = (date: string) => {
    setContextDate(date);
    setTab("context");
  };
```

`<Overview status={status} onOpenDay={openDay} />` and `<DayView date={contextDate ?? undefined} />`.

- [ ] **Step 5: Accept the date in DayView**

In `src/components/DayView.tsx`, change the signature to `export function DayView({ date }: { date?: string } = {})` and add, after the existing `open-day` listener effect:

```tsx
  // The same effect the open-day event has, on an internal route: the
  // Overview map opens a day without going through Tauri.
  useEffect(() => {
    if (date) setSelected(date);
  }, [date]);
```

- [ ] **Step 6: Run the whole suite**

Run: `npx vitest run`
Expected: PASS, every file.

- [ ] **Step 7: Run the CI gates**

Run: `npx tsc --noEmit && npm run build && cd src-tauri && cargo test && cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add src/components/Overview.tsx src/components/Main.tsx src/components/DayView.tsx src/test/Main.test.tsx
git commit -m "feat: put the defrag map on Overview in place of the Status group

The Status group repeated what the status bar already says. Clicking a day
opens it on the Context tab, which DayView reaches the same way the
open-day event does."
```

---

### Task 8: Look at it

Green tests say the logic holds. They say nothing about whether it looks like the reference, which is the whole point of the feature.

**Files:**
- Modify: `src/main-window.css` as the eye requires.

- [ ] **Step 1: Run the app**

Run: `npm run tauri dev`

Open the main window from the menu bar eye, or over the control socket:

```bash
python3 -c "
import socket, json, os
p=os.path.expanduser('~/Library/Application Support/com.0x0000007a.ambientcontext/control.sock')
s=socket.socket(socket.AF_UNIX, socket.SOCK_STREAM); s.connect(p)
s.sendall((json.dumps({'op':'open_day','date':'2026-09-01'})+'\n').encode())
print(s.makefile().readline())"
```

- [ ] **Step 2: Capture the Overview tab**

Raise the app first, or the capture will look right while every click lands on whatever is on top:

```bash
osascript -e 'tell application "System Events" to set frontmost of (first process whose name is "ambient-context") to true'
```

Find the window id and capture it. `CGWindowListCreateImage` was obsoleted in macOS 15, so the capture goes through the command line tool:

```bash
screencapture -x -o -l<window-id> /tmp/overview.png
```

Validate the frame before trusting it: the title bar navy must be present.

- [ ] **Step 3: Compare against the reference**

Put `/tmp/overview.png`, `docs/reference/defrag98-idle.png` and the screenshot named in the spec side by side, and look at all three. Check specifically:

- Cell pitch reads as a lattice, not as separate tiles.
- The four colours match the sampled values on screen.
- The well, the bar trough and the buttons share the chrome's bevel idiom.
- The progress bar's segments are visible as blocks, not a solid fill.
- The info box does not fall outside the well at the right and bottom edges.

- [ ] **Step 4: Adjust and re-capture until it holds**

Fix what the eye finds, and capture again. Do not skip to the commit on the strength of the tests.

- [ ] **Step 5: Commit**

```bash
git add src/main-window.css
git commit -m "fix: settle the defrag map against the reference"
```

---

## Notes for the executor

- **The eye is not delegable.** Task 8 is the gate on this feature. Everything before it proves the logic; only Task 8 proves the look.
- **Summarising costs money.** Every day in a batch is one engine run on the user's own subscription. Do not press Summarise on a real folder to test the batch; the unit tests cover the wiring, and Task 8 only needs the idle state.
- **Do not touch `MAX_BACKFILL_DAYS`** or the scheduled path. The manual button is deliberately uncapped and the scheduler deliberately is not.
