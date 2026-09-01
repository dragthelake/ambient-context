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
  const [cancelled, setCancelled] = useState(0);
  const [status, setStatus] = useState("Ready");
  const running = jobs.length > 0 && done.size < jobs.length;
  const today = useRef(todayIso());

  const reload = useCallback(async () => {
    setDays(await invoke<DayEntry[]>("list_days"));
  }, []);

  useEffect(() => {
    void reload();
    // Capture keeps writing while the window is in the background, so the
    // map is stale by the time it comes back.
    const refresh = () => void reload();
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [reload]);

  const start = useCallback(async () => {
    const dates = pendingDates(days);
    if (dates.length === 0) return;
    setDone(new Set());
    setCancelled(0);
    setFailed(new Set());
    try {
      const ids = await invoke<string[]>("summarise_days", { dates });
      setJobs(ids);
      setStatus(`Summarising ${dates.length} days`);
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
            await reload();
          }
          if (state.status === "failed") {
            setDone((current) => new Set(current).add(job));
            setFailed((current) => new Set(current).add(state.date));
            setStatus(state.stderr ?? `${state.date} failed`);
          }
          if (state.status === "cancelled") {
            setDone((current) => new Set(current).add(job));
            setCancelled((n) => n + 1);
          }
          if (state.status === "running") {
            setStatus(`Summarising ${state.date}`);
          }
        }
      })();
    }, POLL_MS);
    return () => {
      live = false;
      clearInterval(id);
    };
  }, [running, jobs, done, reload]);

  // The batch has finished. Say how it went once, not every tick.
  useEffect(() => {
    if (jobs.length === 0 || done.size < jobs.length) return;
    setStatus(cancelled > 0 ? `Stopped, ${cancelled} skipped` : "Ready");
  }, [jobs, done, cancelled]);

  return {
    days,
    failed,
    today: today.current,
    pending: pendingDates(days),
    running,
    finished: done.size,
    total: jobs.length,
    status,
    start,
    stop,
    reload,
  };
}
