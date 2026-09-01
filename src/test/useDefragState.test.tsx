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

  // Covers the "done" branch's unmount guard. React 19 already turns a
  // setState dispatched on an unmounted fiber into a silent no-op with no
  // observable effect (verified separately: no console.error, no thrown
  // rejection, either way), so this cannot be a test that fails without the
  // fix and passes with it. What it does honestly prove is that resolving
  // reload's own invoke after unmount, which is exactly the path the fix
  // guards, is not left to throw or produce an unhandled rejection.
  it("resolves reload cleanly when it lands after the hook unmounts", async () => {
    let resolveListDays: (value: unknown) => void = () => {};
    const pendingListDays = new Promise((resolve) => {
      resolveListDays = resolve;
    });
    let calls = 0;
    mockInvoke((command) => {
      if (command === "list_days") {
        calls += 1;
        // First call is the mount effect's own reload, which we let
        // resolve immediately; the second is the one we hold open past
        // unmount.
        return calls === 1 ? [] : pendingListDays;
      }
      throw new Error(`unexpected ${command}`);
    });
    let hook: ReturnType<typeof useDefragState> | undefined;
    function Capture() {
      hook = useDefragState();
      return null;
    }
    const { unmount } = render(<Capture />);
    await waitFor(() => expect(calls).toBe(1));
    const inFlight = hook!.reload();
    unmount();
    resolveListDays([]);
    await expect(inFlight).resolves.toBeUndefined();
  });

  it("does not double count a cancelled job when polling ticks overlap", async () => {
    mockInvoke((command) => {
      switch (command) {
        case "list_days":
          return [{ date: "2026-09-01", has_capture: true, has_summary: false, bytes: 1, title: null }];
        case "summarise_days":
          return ["job-0"];
        default:
          throw new Error(`unexpected ${command}`);
      }
    });
    render(<Probe />);
    await waitFor(() =>
      expect(screen.getByTestId("pending").textContent).toBe("2026-09-01"),
    );

    // Fake timers from here on, so the poll effect's setInterval is the one
    // under our control: switching earlier would leave list_days' own
    // microtask resolution racing waitFor's own timer-based polling.
    vi.useFakeTimers();
    const jobStateResolvers: Array<(value: unknown) => void> = [];
    mockInvoke((command) => {
      switch (command) {
        case "list_days":
          return [{ date: "2026-09-01", has_capture: true, has_summary: false, bytes: 1, title: null }];
        case "summarise_days":
          return ["job-0"];
        case "job_state":
          return new Promise((resolve) => {
            jobStateResolvers.push(resolve);
          });
        default:
          throw new Error(`unexpected ${command}`);
      }
    });

    await act(async () => {
      screen.getByText("start").click();
    });

    // Neither job_state call resolves before the next tick, so the poll
    // effect's interval is never torn down between them: both ticks read
    // the same outstanding job from the same stale closure and each issues
    // its own job_state call for it.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(jobStateResolvers).toHaveLength(2);

    // Both overlapping ticks learn the job was cancelled and race to record
    // it. A Set dedupes the id; an increment does not.
    await act(async () => {
      jobStateResolvers.forEach((resolve) =>
        resolve({ id: "job-0", date: "2026-09-01", status: "cancelled", stderr: null }),
      );
      await vi.advanceTimersByTimeAsync(0);
    });

    expect(screen.getByTestId("status").textContent).toBe("Stopped, 1 skipped");
    vi.useRealTimers();
  });
});
