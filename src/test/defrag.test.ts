import { describe, expect, it } from "vitest";
import { buildCells, MIN_ROWS, pendingDates, stateOf } from "../lib/defrag";
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

  it("builds the empty rectangle when no day has been recorded", () => {
    // Zero days is the case MIN_ROWS exists for at its extreme: returning
    // nothing collapses the panel to the height of its own padding.
    const cells = buildCells([], "2026-09-01", 10, new Set());
    expect(cells).toHaveLength(MIN_ROWS * 10);
    expect(cells.every((cell) => cell.state === "empty" && cell.entry === null)).toBe(true);
  });

  it("has no cells to lay out when the well has not been measured yet", () => {
    expect(buildCells([entry("2026-09-01")], "2026-09-01", 0, new Set())).toEqual([]);
  });
});

describe("stateOf", () => {
  it("counts a summary as summarised even with the raw file gone", () => {
    const day = entry("2026-09-01", { has_capture: false, has_summary: true });
    expect(stateOf(day, false)).toBe("summarised");
  });

  it("is empty for a day with neither file, and for no day at all", () => {
    expect(stateOf(entry("2026-09-01", { has_capture: false }), false)).toBe("empty");
    expect(stateOf(null, false)).toBe("empty");
  });

  it("puts a failed attempt above whatever the files say", () => {
    expect(stateOf(entry("2026-09-01", { has_summary: true }), true)).toBe("failed");
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
