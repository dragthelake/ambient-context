import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { callsOf, countOf, emit, mockInvoke } from "./tauri-mock";
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

function handler(
  pendingDay: string | null,
  summary: string | null = null,
  knowledge: string | null = null,
) {
  return (command: string) => {
    switch (command) {
      case "take_pending_day":
        return pendingDay;
      case "list_days":
        return [
          {
            date: todayIso(),
            has_capture: true,
            has_summary: summary !== null,
            has_kb: knowledge !== null,
            bytes: 10,
            title: null,
          },
        ];
      case "read_day":
        return "## 09:00-09:30 Finder\n\nsomething\n";
      case "read_summary":
        return summary;
      case "get_settings":
        return { agent: { label: "Claude Code" } };
      case "job_status":
        // A fresh object every call, as the real command returns.
        return {
          when: "2026-08-30T06:00:00+10:00",
          date: todayIso(),
          ok: true,
          message: "done",
          took_ms: 252000,
        };
      case "open_in_editor":
        return null;
      case "read_day_blocks":
        return [];
      case "website_totals":
        return [];
      case "get_rules":
        return { rules: [], built_ins: [], next_id: "r1", error: null };
      case "read_kb":
        return knowledge;
      case "summarise_now":
      case "ingest_now":
        return { job_id: "job-9" };
      case "job_state":
        return {
          id: "job-9",
          date: todayIso(),
          status: "running",
          stderr: null,
          step: "Reading apps (2 of 3)",
        };
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

  it("takes the pending day again after an open-day event, so a remount cannot replay it", async () => {
    mockInvoke(handler(null));
    render(<DayView />);
    await waitFor(() => expect(countOf("take_pending_day")).toBe(1));
    await waitFor(() => expect(countOf("list_days")).toBeGreaterThan(0));
    emit("open-day", "2026-07-04");
    expect(await screen.findByText(/4 July 2026/)).toBeTruthy();
    await waitFor(() => expect(countOf("take_pending_day")).toBe(2));
  });

  it("offers three modes, one action under the box, and nothing about ingesting", async () => {
    mockInvoke(handler(null));
    render(<DayView />);
    await waitFor(() => expect(countOf("list_days")).toBeGreaterThan(0));
    for (const label of ["Context", "Knowledge", "Notes"]) {
      expect(screen.getByRole("tab", { name: label })).toBeTruthy();
    }
    expect(screen.queryByText(/Ingest/i)).toBeNull();
    expect(screen.queryByText(/Summaris/i)).toBeNull();
    expect(await screen.findByText(/0\.5 h recorded · 1 block · No notes yet/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Process day" })).toBeTruthy();
  });

  it("processes a day that has no notes and shows the step text", async () => {
    mockInvoke(handler(null));
    render(<DayView />);
    await waitFor(() => expect(countOf("list_days")).toBeGreaterThan(0));
    fireEvent.click(screen.getByRole("button", { name: "Process day" }));
    await waitFor(() => expect(callsOf("summarise_now")[0]?.args?.force).toBe(false));
    await waitFor(() => expect(screen.getByText("Reading apps (2 of 3)…")).toBeTruthy(), {
      timeout: 3000,
    });
  }, 10000);

  it("reprocesses a day that already has notes, and says when they were written", async () => {
    mockInvoke(handler(null, "---\ndate: today\n---\n\n# Day\n\nSomething happened.\n"));
    render(<DayView />);
    await waitFor(() => expect(countOf("list_days")).toBeGreaterThan(0));
    expect(await screen.findByText(/Notes 06:00, took 4 min/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Reprocess day" }));
    await waitFor(() => expect(callsOf("summarise_now")[0]?.args?.force).toBe(true));
  });

  it("generates only the knowledge from the Knowledge tab", async () => {
    mockInvoke(handler(null));
    render(<DayView />);
    await waitFor(() => expect(countOf("list_days")).toBeGreaterThan(0));
    fireEvent.click(screen.getByRole("tab", { name: "Knowledge" }));
    expect(await screen.findByText("Nothing built for this day yet.")).toBeTruthy();
    // One Generate under the box and one in the empty state, both the same run.
    const buttons = screen.getAllByRole("button", { name: "Generate" });
    expect(buttons).toHaveLength(2);
    fireEvent.click(buttons[0]);
    await waitFor(() => expect(callsOf("ingest_now")[0]?.args?.force).toBe(false));
    expect(callsOf("summarise_now")).toHaveLength(0);
  });

  it("shows one knowledge section at a time, chosen from the strip", async () => {
    const knowledge = "# people.md\n\n## Dan\nAsked for the notch\n\n# issues.md\n\n- the eye stutters\n";
    mockInvoke(handler(null, null, knowledge));
    render(<DayView />);
    await waitFor(() => expect(countOf("list_days")).toBeGreaterThan(0));
    fireEvent.click(screen.getByRole("tab", { name: "Knowledge" }));
    expect(await screen.findByText("Dan")).toBeTruthy();
    expect(screen.queryByText("the eye stutters")).toBeNull();
    fireEvent.click(screen.getByRole("tab", { name: "Issues" }));
    expect(await screen.findByText("the eye stutters")).toBeTruthy();
    expect(screen.queryByText("Dan")).toBeNull();
    expect(screen.getByRole("button", { name: "Regenerate" })).toBeTruthy();
  });

  it("opens the knowledge section on screen, not always the first one", async () => {
    // The command used to resolve every section to threads.md, so five of
    // the six opened a file the reader was not looking at.
    const knowledge = "# people.md\n\n## Dan\nAsked for the notch\n\n# issues.md\n\n- the eye stutters\n";
    mockInvoke(handler(null, null, knowledge));
    render(<DayView />);
    await waitFor(() => expect(countOf("list_days")).toBeGreaterThan(0));
    fireEvent.click(screen.getByRole("tab", { name: "Knowledge" }));
    fireEvent.click(await screen.findByRole("tab", { name: "Issues" }));
    fireEvent.click(screen.getByRole("button", { name: "Open in editor" }));
    await waitFor(() => expect(countOf("open_in_editor")).toBe(1));
    expect(callsOf("open_in_editor")[0].args?.which).toBe("kb/issues.md");
  });

  it("leaves the day alone while a textarea has the keystroke", async () => {
    // The propose popover renders a textarea inside this view, and the day
    // shortcuts listen on the window, so typing "t" used to jump to today
    // and the arrow keys used to move a day.
    mockInvoke(handler("2026-07-04"));
    render(<DayView />);
    expect(await screen.findByText(/4 July 2026/)).toBeTruthy();
    const typing = document.createElement("textarea");
    document.body.appendChild(typing);
    fireEvent.keyDown(typing, { key: "t" });
    fireEvent.keyDown(typing, { key: "ArrowRight" });
    expect(screen.getByText(/4 July 2026/)).toBeTruthy();
    // The same keys still work when nothing is being typed into.
    fireEvent.keyDown(document.body, { key: "ArrowRight" });
    expect(await screen.findByText(/5 July 2026/)).toBeTruthy();
    typing.remove();
  });

  it("switches the record pane between apps, websites and messages", async () => {
    mockInvoke(handler(null));
    render(<DayView />);
    await waitFor(() => expect(countOf("read_day_blocks")).toBeGreaterThan(0));
    fireEvent.click(screen.getByRole("tab", { name: "Websites" }));
    await waitFor(() => expect(countOf("website_totals")).toBeGreaterThan(0));
    fireEvent.click(screen.getByRole("tab", { name: "Messages" }));
    await waitFor(() =>
      expect(callsOf("read_day_blocks").some((call) => call.args?.file === "messages")).toBe(
        true,
      ),
    );
  });
});
