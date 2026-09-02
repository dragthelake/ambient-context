import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DefragMap } from "../components/DefragMap";
import { MIN_ROWS } from "../lib/defrag";
import type { DayEntry } from "../lib/days";

afterEach(cleanup);

function entry(date: string, over: Partial<DayEntry> = {}): DayEntry {
  return { date, has_capture: true, has_summary: false, has_kb: false, bytes: 2048, title: null, ...over };
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

  it("holds the panel's shape and says why it is empty with nothing recorded", () => {
    render(
      <DefragMap days={[]} failed={new Set()} today="2026-09-01" onOpenDay={vi.fn()} />,
    );
    // The floor exists so the panel keeps its shape when little has been
    // recorded, and a brand new install is that case at its extreme. It
    // collapsing to a sliver is what a first-run user would see.
    const cells = screen.getAllByRole("button");
    expect(cells).toHaveLength(MIN_ROWS);
    expect(cells.every((cell) => (cell as HTMLButtonElement).disabled)).toBe(true);
    expect(screen.getByText(/Nothing recorded yet/)).toBeTruthy();
  });

  it("treats a day with a summary but no raw file as summarised", () => {
    const onOpenDay = vi.fn();
    render(
      <DefragMap
        days={[entry("2026-09-01", { has_capture: false, has_summary: true, bytes: 0 })]}
        failed={new Set()}
        today="2026-09-01"
        onOpenDay={onOpenDay}
      />,
    );
    const cell = screen.getByRole("button", { name: /1 September 2026/ });
    expect((cell as HTMLButtonElement).disabled).toBe(false);
    expect(cell.className).toContain("is-summarised");

    // The raw file is gone, so there is no size to report: 0 B here reads
    // as an empty day rather than a summarised one.
    fireEvent.mouseEnter(cell);
    const info = screen.getByRole("tooltip").textContent ?? "";
    expect(info).toContain("Processed");
    expect(info).not.toContain("0 B");

    fireEvent.click(cell);
    expect(onOpenDay).toHaveBeenCalledWith("2026-09-01");
  });
});
