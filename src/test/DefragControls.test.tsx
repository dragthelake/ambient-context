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
