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
