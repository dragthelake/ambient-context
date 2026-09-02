import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { pendingDates } from "./defrag";
import type { DayEntry } from "./days";

type JobState = {
  id: string;
  date: string;
  status: "queued" | "running" | "done" | "failed" | "cancelled";
  stderr: string | null;
};

/// The same cadence DayView uses for its own on-demand run.
const POLL_MS = 2000;

function todayIso(): string {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
}

/// Owns the day list, the failed dates and the batch. Both the map and the
/// controls are handed what they draw, because the map needs the failed set
/// to colour cells and the controls need the same batch to fill the bar:
/// splitting the state between them would mean passing it sideways.
export function useDefragState() {
  const [days, setDays] = useState<DayEntry[]>([]);
  const [failed, setFailed] = useState<Set<string>>(new Set());
  const [jobs, setJobs] = useState<string[]>([]);
  const [done, setDone] = useState<Set<string>>(new Set());
  const [cancelled, setCancelled] = useState<Set<string>>(new Set());
  const [status, setStatus] = useState("Ready");
  // The date whose agent run is under way right now, so the map can blink
  // its cell the way the reference blinks the block being worked on.
  const [active, setActive] = useState<string | null>(null);
  const running = jobs.length > 0 && done.size < jobs.length;
  // Recomputed on every reload rather than fixed at mount. The window is
  // long lived and sits open across midnight, and a frozen `today` stops
  // the map short of the day being captured right now.
  const [today, setToday] = useState(todayIso());
  // The first failure of the batch, which is the one the status line
  // reports. Later failures are counted, not quoted: the first is the one
  // that explains the run, and the last is usually the same message again.
  const firstFailure = useRef<string | null>(null);
  // Shared by every caller of reload, not just the poll effect's own local
  // `live` flag: reload is also called from the mount effect and the focus
  // listener, and any of those awaits can still be in flight at unmount.
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const reload = useCallback(async () => {
    const nextDays = await invoke<DayEntry[]>("list_days");
    if (!mounted.current) return;
    setToday(todayIso());
    setDays(nextDays);
  }, []);

  useEffect(() => {
    void reload();
    // Capture keeps writing while the window is in the background, so the
    // map is stale by the time it comes back.
    const refresh = () => void reload();
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [reload]);

  // The runner outlives the window: closing it mid-batch destroys the
  // webview and nothing else. Adopting whatever is still queued or running
  // means Stop is reachable on reopen and Summarise does not offer to
  // enqueue days that are already on their way, which the user pays for
  // twice. The failed dates of the earlier batch are not recoverable this
  // way and are not attempted.
  useEffect(() => {
    void (async () => {
      const ids = await invoke<string[]>("running_batch");
      if (!mounted.current || ids.length === 0) return;
      setJobs(ids);
      setStatus(`Processing ${ids.length === 1 ? "1 day" : `${ids.length} days`}`);
    })();
  }, []);

  const start = useCallback(async () => {
    const dates = pendingDates(days);
    if (dates.length === 0) return;
    // `jobs` too, not only the counters: `running` is derived from the two
    // together, so leaving the previous batch's ids in place while
    // summarise_days is rejected would leave the hook running against a
    // batch that finished, polling every one of those dead ids.
    setJobs([]);
    setDone(new Set());
    setCancelled(new Set());
    setFailed(new Set());
    firstFailure.current = null;
    try {
      const ids = await invoke<string[]>("summarise_days", { dates });
      setJobs(ids);
      setStatus(`Processing ${dates.length === 1 ? "1 day" : `${dates.length} days`}`);
    } catch (error) {
      setStatus(String(error));
    }
  }, [days]);

  const stop = useCallback(async () => {
    await invoke<number>("cancel_queued_summaries");
    setStatus("Stopping");
  }, []);

  // Poll only what is outstanding. A finished job never changes again, so
  // asking after it is a request per job per tick for no new information.
  useEffect(() => {
    if (!running) return;
    let live = true;
    const id = setInterval(() => {
      void (async () => {
        const outstanding = jobs.filter((job) => !done.has(job));
        for (const job of outstanding) {
          const state = await invoke<JobState | null>("job_state", { jobId: job });
          if (!live || !state) continue;
          if (state.status === "done") {
            setDone((current) => new Set(current).add(job));
            // reload does its own invoke round trip; it guards its own
            // setDays against the hook having unmounted meanwhile, since
            // the `live` flag here only covers this tick, not that second
            // await.
            await reload();
          }
          if (state.status === "failed") {
            setDone((current) => new Set(current).add(job));
            setFailed((current) => new Set(current).add(state.date));
            if (firstFailure.current === null) {
              firstFailure.current = state.stderr ?? `${state.date} failed`;
              setStatus(firstFailure.current);
            }
          }
          if (state.status === "cancelled") {
            setDone((current) => new Set(current).add(job));
            // A job id added twice, from overlapping ticks racing on the
            // same job, is still one id: the count must come from the
            // set's size, never from an increment that can double count.
            setCancelled((current) => new Set(current).add(job));
          }
          if (state.status === "running") {
            setStatus(`Processing ${state.date}`);
            setActive(state.date);
          } else {
            // Whatever this job's terminal state, its cell stops working.
            setActive((current) => (current === state.date ? null : current));
          }
        }
      })();
    }, POLL_MS);
    return () => {
      live = false;
      clearInterval(id);
    };
  }, [running, jobs, done, reload]);

  // The batch has finished. Say how it went once, not every tick. Failures
  // land in `done` so the bar can reach the end, which means "every job
  // finished" is not "every job worked": without the failed set consulted
  // here, a run where nothing was summarised would report Ready beside a
  // bar reading 100%.
  useEffect(() => {
    if (jobs.length === 0 || done.size < jobs.length) return;
    setActive(null);
    if (cancelled.size > 0) {
      setStatus(`Stopped, ${cancelled.size} skipped`);
      return;
    }
    if (failed.size > 0) {
      const reason = firstFailure.current ? `: ${firstFailure.current}` : "";
      setStatus(`Finished, ${failed.size} failed${reason}`);
      return;
    }
    setStatus("Ready");
  }, [jobs, done, cancelled, failed]);

  return {
    days,
    failed,
    today,
    pending: pendingDates(days),
    running,
    active,
    finished: done.size,
    total: jobs.length,
    status,
    start,
    stop,
    reload,
  };
}
