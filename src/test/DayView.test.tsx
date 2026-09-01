import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { callsOf, countOf, mockInvoke } from "./tauri-mock";
import { DayView } from "../components/DayView";

vi.mock("@tauri-apps/api/core", async () => {
  const mock = await import("./tauri-mock");
  return { invoke: mock.invoke };
});
vi.mock("@tauri-apps/api/event", async () => {
  const mock = await import("./tauri-mock");
  return { listen: mock.listen };
});

function todayIso(): string {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
}

function handler(pendingDay: string | null) {
  return (command: string) => {
    switch (command) {
      case "take_pending_day":
        return pendingDay;
      case "list_days":
        return [];
      case "read_day":
        return "## 09:00-09:30 Finder\n\nsomething\n";
      case "read_summary":
        return null;
      case "get_settings":
        return { engine: null };
      case "job_status":
        // A fresh object every call, as the real command returns.
        return { when: "2026-08-30T06:00:00+10:00", date: todayIso(), ok: true, message: "done" };
      case "read_day_blocks":
        return [];
      case "get_rules":
        return { rules: [], built_ins: [], next_id: "r1", error: null };
      default:
        throw new Error(`unexpected command ${command}`);
    }
  };
}

async function settle(): Promise<void> {
  for (let i = 0; i < 20; i += 1) await Promise.resolve();
  await new Promise((resolve) => setTimeout(resolve, 0));
  for (let i = 0; i < 20; i += 1) await Promise.resolve();
}

describe("DayView", () => {
  beforeEach(() => {
    vi.useRealTimers();
  });
  afterEach(cleanup);

  it("reads the day a bounded number of times when a job outcome exists", async () => {
    mockInvoke(handler(null));
    render(<DayView />);
    await waitFor(() => expect(countOf("job_status")).toBeGreaterThan(0));
    await settle();
    await settle();
    // Before the fix the outcome fed its own effect and this grew without end.
    expect(countOf("read_day")).toBeLessThanOrEqual(2);
    expect(countOf("job_status")).toBeLessThanOrEqual(2);
  });

  it("selects the day take_pending_day hands back", async () => {
    mockInvoke(handler("2026-07-04"));
    render(<DayView />);
    await waitFor(() =>
      expect(callsOf("read_day").some((call) => call.args?.date === "2026-07-04")).toBe(true),
    );
    expect(await screen.findByText(/4 July 2026/)).toBeTruthy();
  });
});
