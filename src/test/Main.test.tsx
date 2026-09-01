import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { mockInvoke } from "./tauri-mock";
import { Main } from "../components/Main";
import type { DayEntry } from "../lib/days";

// A single recorded day, fixed to whatever "today" is when the suite runs.
// The map draws every day from the first recorded one through today, so
// today is always in range; deriving the expected label from this same
// value (rather than writing a month name as a literal) keeps the test
// from going stale the next time the calendar turns over.
const RECORDED_DATE = (() => {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
})();

const RECORDED_LABEL = new Date(`${RECORDED_DATE}T00:00:00Z`).toLocaleDateString(
  "en-AU",
  { day: "numeric", month: "long", year: "numeric", timeZone: "UTC" },
);

// A second recorded day, far enough back to land in an earlier month than
// today for any day of the year, and used by the tab-reset and cross-month
// tests below. Ninety days keeps it clear of a run happening near the
// start of a month, which forty or so days back would not.
const OLDER_DATE = (() => {
  const now = new Date();
  now.setDate(now.getDate() - 90);
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
})();

const OLDER_LABEL = new Date(`${OLDER_DATE}T00:00:00Z`).toLocaleDateString(
  "en-AU",
  { day: "numeric", month: "long", year: "numeric", timeZone: "UTC" },
);

// DayHeader's own date heading, used to prove which day the Context pane
// landed on.
function dayHeading(iso: string): string {
  const [y, m, d] = iso.split("-").map(Number);
  return new Date(y, m - 1, d).toLocaleDateString("en-AU", {
    weekday: "long",
    day: "numeric",
    month: "long",
    year: "numeric",
  });
}

const TODAY_HEADING = dayHeading(RECORDED_DATE);
const OLDER_HEADING = dayHeading(OLDER_DATE);


vi.mock("@tauri-apps/api/core", async () => {
  const mock = await import("./tauri-mock");
  return { invoke: mock.invoke };
});
vi.mock("@tauri-apps/api/event", async () => {
  const mock = await import("./tauri-mock");
  return { listen: mock.listen };
});
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ close: () => undefined }),
}));

// The Overview tab's commands, plus the Context tab's: clicking a map cell
// switches to Context and mounts DayView for real, so its own mount-time
// commands need answers too, or they reject unhandled once the test moves
// on.
function handler(command: string) {
  switch (command) {
    case "capture_status":
      return { running: false, blocks_today: 0 };
    case "permission_status":
      return "granted";
    case "current_folder":
      return "/Users/someone/Ambient Context";
    case "get_settings":
      // The union of keys Overview, Main and DayView each read on mount, plus
      // the Settings tab's own panels once the Agent tab test switches to it.
      return {
        sound_enabled: true,
        sound_volume: 0.6,
        agent: null,
        extra_redaction_patterns: [],
      };
    case "mcp_registration":
      return {
        binary: "/usr/local/bin/ambient-context",
        quoted_binary: "/usr/local/bin/ambient-context",
        running: false,
        last_write: null,
      };
    case "agent_detect":
      return [];
    case "get_prompt":
      return { text: "", customised: false, path: "/tmp/prompt.md" };
    case "list_days":
      return [
        {
          date: RECORDED_DATE,
          has_capture: true,
          has_summary: false,
          bytes: 128,
          title: null,
        } satisfies DayEntry,
        {
          date: OLDER_DATE,
          has_capture: true,
          has_summary: false,
          bytes: 96,
          title: null,
        } satisfies DayEntry,
      ];
    case "summarise_days":
      return [];
    case "cancel_queued_summaries":
      return 0;
    case "running_batch":
      return [];
    case "take_pending_day":
      return null;
    case "job_status":
      return null;
    case "read_day":
      return null;
    case "read_summary":
      return null;
    case "read_day_blocks":
      return [];
    case "get_rules":
      return { rules: [], built_ins: [], next_id: "r1", error: null };
    default:
      throw new Error(`unexpected command ${command}`);
  }
}

afterEach(cleanup);

describe("the main window's tab strip", () => {
  it("opens on Overview", () => {
    mockInvoke(handler);
    render(<Main />);

    const overview = screen.getByRole("tab", { name: "Overview" });
    expect(overview.getAttribute("aria-selected")).toBe("true");
    for (const name of ["Context", "Settings"]) {
      expect(
        screen.getByRole("tab", { name }).getAttribute("aria-selected"),
      ).toBe("false");
    }
  });

  it("shows only the chosen tab's pane", () => {
    mockInvoke(handler);
    render(<Main />);

    // The record toggle belongs to Overview and nothing else, so its
    // presence is what "the Overview pane is the one showing" means.
    expect(screen.getAllByRole("tabpanel")).toHaveLength(1);
    expect(screen.getByRole("button", { name: /recording/i })).toBeTruthy();
  });

  it("opens a day on the Context tab when a map cell is clicked", async () => {
    mockInvoke(handler);
    render(<Main />);
    await waitFor(() =>
      expect(
        screen.getAllByRole("button", { name: RECORDED_LABEL }).length,
      ).toBeGreaterThan(0),
    );
    fireEvent.click(screen.getAllByRole("button", { name: RECORDED_LABEL })[0]);
    await waitFor(() =>
      expect(
        screen.getByRole("tab", { name: "Context" }).getAttribute("aria-selected"),
      ).toBe("true"),
    );
  });

  it("does not reopen a clicked day when the Context tab is pressed directly afterwards", async () => {
    mockInvoke(handler);
    render(<Main />);

    // Open OLDER_DATE from the map, and confirm the Context pane landed on it.
    await waitFor(() =>
      expect(
        screen.getAllByRole("button", { name: OLDER_LABEL }).length,
      ).toBeGreaterThan(0),
    );
    fireEvent.click(screen.getAllByRole("button", { name: OLDER_LABEL })[0]);
    await waitFor(() =>
      expect(screen.getByText(OLDER_HEADING)).toBeTruthy(),
    );

    // Leave, then come back to Context by pressing its tab directly, not
    // through the map. The day that click opened must not still be sticking.
    fireEvent.click(screen.getByRole("tab", { name: "Overview" }));
    fireEvent.click(screen.getByRole("tab", { name: "Context" }));

    await waitFor(() => expect(screen.getByText(TODAY_HEADING)).toBeTruthy());
    expect(screen.queryByText(OLDER_HEADING)).toBe(null);
  });

  it("opens a day from an earlier month when its cell is clicked", async () => {
    mockInvoke(handler);
    render(<Main />);

    await waitFor(() =>
      expect(
        screen.getAllByRole("button", { name: OLDER_LABEL }).length,
      ).toBeGreaterThan(0),
    );
    fireEvent.click(screen.getAllByRole("button", { name: OLDER_LABEL })[0]);

    // A day 90 days back is always in a different month from today, which is
    // the case the calendar rail used to have to be kept in step with. The
    // rail is gone; landing on the right day still has to hold.
    await waitFor(() => expect(screen.getByText(OLDER_HEADING)).toBeTruthy());
  });

  it("has an Agent tab between Context and Settings", async () => {
    mockInvoke(handler);
    render(<Main />);
    const tabs = screen.getAllByRole("tab").map((t) => t.textContent);
    expect(tabs).toEqual(["Overview", "Context", "Agent", "Settings"]);
  });

  it("shows the agent options on the Agent tab, not in Settings", async () => {
    mockInvoke(handler);
    render(<Main />);
    fireEvent.click(screen.getByRole("tab", { name: "Agent" }));
    expect(await screen.findByText("Schedule")).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "Settings" }));
    await waitFor(() => expect(screen.queryByText("Schedule")).toBe(null));
  });
});
