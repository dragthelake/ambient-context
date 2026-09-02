import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { processedDays, RecentNotes } from "../components/RecentNotes";
import type { DayEntry } from "../lib/days";

function day(over: Partial<DayEntry> & Pick<DayEntry, "date">): DayEntry {
  return {
    has_capture: true,
    has_summary: false,
    has_kb: false,
    bytes: 100,
    title: null,
    ...over,
  };
}

describe("processedDays", () => {
  it("keeps only summarised days, newest first, capped", () => {
    const days = [
      day({ date: "2026-09-03", has_summary: true, title: "Today" }),
      day({ date: "2026-09-02", has_summary: false }),
      day({ date: "2026-09-01", has_summary: true, title: "Tuesday" }),
    ];
    expect(processedDays(days).map((d) => d.date)).toEqual([
      "2026-09-03",
      "2026-09-01",
    ]);
  });
});

describe("RecentNotes", () => {
  afterEach(cleanup);

  it("lists processed days and opens one on click", () => {
    const onOpenDay = vi.fn();
    render(
      <RecentNotes
        hasAgent
        onOpenDay={onOpenDay}
        days={[
          day({ date: "2026-09-01", has_summary: true, title: "A day of plumbing" }),
          day({ date: "2026-08-31", has_summary: false }),
        ]}
      />,
    );
    expect(screen.getByText("A day of plumbing")).toBeTruthy();
    expect(screen.queryByText("2026-08-31")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /A day of plumbing/ }));
    expect(onOpenDay).toHaveBeenCalledWith("2026-09-01");
  });

  it("explains an empty record", () => {
    render(<RecentNotes days={[]} hasAgent onOpenDay={() => {}} />);
    expect(screen.getByText("Nothing recorded yet.")).toBeTruthy();
  });

  it("explains a record with no notes yet", () => {
    render(
      <RecentNotes
        hasAgent
        onOpenDay={() => {}}
        days={[day({ date: "2026-09-01" })]}
      />,
    );
    expect(screen.getByText("No notes yet.")).toBeTruthy();
    expect(
      screen.getByText(/Process one to write a short note/),
    ).toBeTruthy();
  });
});
