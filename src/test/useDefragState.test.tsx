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
      <span data-testid="running">{String(state.running)}</span>
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
      if (command === "running_batch") return [];
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
        case "running_batch":
          return [];
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
        case "running_batch":
          return [];
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
      if (command === "running_batch") return [];
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

  it("reports the count and the first failure when jobs in a batch fail", async () => {
    mockInvoke((command, args) => {
      switch (command) {
        case "list_days":
          return [
            { date: "2026-08-31", has_capture: true, has_summary: false, bytes: 1, title: null },
            { date: "2026-09-01", has_capture: true, has_summary: false, bytes: 1, title: null },
          ];
        case "running_batch":
          return [];
        case "summarise_days":
          return ["job-0", "job-1"];
        case "job_state":
          return args?.jobId === "job-0"
            ? { id: "job-0", date: "2026-08-31", status: "failed", stderr: "agent died" }
            : { id: "job-1", date: "2026-09-01", status: "failed", stderr: "and again" };
        default:
          throw new Error(`unexpected ${command}`);
      }
    });
    render(<Probe />);
    await waitFor(() =>
      expect(screen.getByTestId("pending").textContent).toBe("2026-08-31,2026-09-01"),
    );

    vi.useFakeTimers();
    await act(async () => {
      screen.getByText("start").click();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });

    // Failures count towards completion so the bar can reach the end, which
    // means "every job finished" is not "every job worked". A run that
    // summarised nothing must not report Ready beside a full bar, and the
    // failure quoted is the first, which is the one that explains the run.
    expect(screen.getByTestId("progress").textContent).toBe("2/2");
    expect(screen.getByTestId("status").textContent).toBe(
      "Finished, 2 failed: agent died",
    );
    vi.useRealTimers();
  });

  it("clears the last batch when starting the next one is rejected", async () => {
    let rejecting = false;
    mockInvoke((command) => {
      switch (command) {
        case "list_days":
          return [{ date: "2026-09-01", has_capture: true, has_summary: false, bytes: 1, title: null }];
        case "running_batch":
          return [];
        case "summarise_days":
          if (rejecting) throw new Error("no agent is connected");
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
    expect(screen.getByTestId("progress").textContent).toBe("0/1");

    // A rejected start never sets new ids. Leaving the last batch's ids in
    // place leaves `running` true against jobs that already finished, which
    // enables Stop, disables Summarise, and polls every dead id.
    rejecting = true;
    await act(async () => {
      screen.getByText("start").click();
    });
    expect(screen.getByTestId("progress").textContent).toBe("0/0");
    expect(screen.getByTestId("running").textContent).toBe("false");
    expect(screen.getByTestId("status").textContent).toContain("no agent is connected");
  });

  it("adopts a batch that is still running when the window opens", async () => {
    mockInvoke((command) => {
      switch (command) {
        case "list_days":
          return [{ date: "2026-09-01", has_capture: true, has_summary: false, bytes: 1, title: null }];
        case "running_batch":
          return ["job-4", "job-5"];
        case "job_state":
          return { id: "job-4", date: "2026-09-01", status: "running", stderr: null };
        default:
          throw new Error(`unexpected ${command}`);
      }
    });
    render(<Probe />);
    // The runner outlives the window, so a window reopened mid-batch has to
    // pick the batch back up: Stop is unreachable otherwise, and Summarise
    // would offer to enqueue days that are already on their way.
    await waitFor(() => expect(screen.getByTestId("progress").textContent).toBe("0/2"));
    expect(screen.getByTestId("running").textContent).toBe("true");
    expect(screen.getByTestId("status").textContent).toBe("Summarising 2 days");
  });

  it("recomputes today when the day list reloads", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(new Date(2026, 8, 1, 23, 30));
    mockInvoke((command) => {
      if (command === "list_days") return [];
      if (command === "running_batch") return [];
      throw new Error(`unexpected ${command}`);
    });
    let hook: ReturnType<typeof useDefragState> | undefined;
    function Capture() {
      hook = useDefragState();
      return null;
    }
    render(<Capture />);
    await waitFor(() => expect(hook!.today).toBe("2026-09-01"));

    // The window sits open across midnight. A `today` fixed at mount stops
    // the map one cell short of the day being captured right now.
    vi.setSystemTime(new Date(2026, 8, 2, 0, 30));
    await act(async () => {
      await hook!.reload();
    });
    expect(hook!.today).toBe("2026-09-02");
    vi.useRealTimers();
  });

  it("does not double count a cancelled job when polling ticks overlap", async () => {
    mockInvoke((command) => {
      switch (command) {
        case "list_days":
          return [{ date: "2026-09-01", has_capture: true, has_summary: false, bytes: 1, title: null }];
        case "summarise_days":
          return ["job-0"];
        case "running_batch":
          return [];
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
        case "running_batch":
          return [];
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
