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
