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
///
/// With nothing recorded at all the result is the same empty rectangle
/// rather than nothing: the floor exists so the panel holds its shape when
/// little has been recorded, and a brand new install is that case at its
/// extreme.
export function buildCells(
  days: DayEntry[],
  today: string,
  columns: number,
  failed: Set<string>,
): Cell[] {
  if (columns <= 0) return [];

  const cells: Cell[] = [];
  if (days.length > 0) {
    const byDate = new Map(days.map((day) => [day.date, day]));
    const first = days.reduce(
      (earliest, day) => (day.date < earliest ? day.date : earliest),
      days[0].date,
    );
    for (let at = parse(first); iso(at) <= today; at = addDays(at, 1)) {
      const date = iso(at);
      const entry = byDate.get(date) ?? null;
      cells.push({ date, entry, state: stateOf(entry, failed.has(date)) });
    }
  }

  const rows = Math.max(MIN_ROWS, Math.ceil(cells.length / columns));
  const total = rows * columns;
  for (let n = cells.length; n < total; n += 1) {
    cells.push({ date: "", state: "empty", entry: null });
  }
  return cells;
}

/// A summary counts as summarised whether or not the raw day file is still
/// there. The backend lists a date as soon as either file exists, so a day
/// whose raw context has been deleted after summarising is a real shape,
/// and drawing it white would say nothing was ever recorded.
export function stateOf(entry: DayEntry | null, hasFailed: boolean): CellState {
  if (!entry) return "empty";
  if (hasFailed) return "failed";
  if (entry.has_summary) return "summarised";
  return entry.has_capture ? "raw" : "empty";
}

/// Days holding raw context with no summary, oldest first, which is the
/// order the runner works through them.
export function pendingDates(days: DayEntry[]): string[] {
  return days
    .filter((day) => day.has_capture && !day.has_summary)
    .map((day) => day.date)
    .sort();
}
