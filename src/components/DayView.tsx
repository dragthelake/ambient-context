import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { CalendarRail } from "./CalendarRail";
import { DayHeader } from "./DayHeader";
import { RawPane } from "./RawPane";
import { SummaryPane } from "./SummaryPane";
import type { DayEntry } from "./CalendarRail";

export type { DayEntry };

export type DayStats = { blocks: number; hours: number };

export type Outcome = {
  when: string;
  date: string;
  ok: boolean;
  message: string;
};

/// Compare two outcomes by value: `job_status` returns a fresh object every
/// call, and setting an equal-but-new one re-renders the whole day for nothing.
function sameOutcome(a: Outcome | null, b: Outcome | null): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  return (
    a.when === b.when &&
    a.date === b.date &&
    a.ok === b.ok &&
    a.message === b.message
  );
}

export type JobStatus = "queued" | "running" | "done" | "failed";

export type JobState = {
  id: string;
  date: string;
  status: JobStatus;
  stderr: string | null;
};

export type SummaryState =
  | { kind: "none" }
  | { kind: "queued" }
  | { kind: "running" }
  | { kind: "generated"; at: string }
  | { kind: "failed"; message: string };

const BLOCK_HEADING = /^## (\d{2}):(\d{2})[-–](\d{2}):(\d{2})/;

export function dayStats(dayMarkdown: string | null): DayStats {
  if (!dayMarkdown) return { blocks: 0, hours: 0 };
  let blocks = 0;
  let minutes = 0;
  for (const line of dayMarkdown.split("\n")) {
    const match = BLOCK_HEADING.exec(line);
    if (!match) continue;
    blocks += 1;
    const start = Number(match[1]) * 60 + Number(match[2]);
    const end = Number(match[3]) * 60 + Number(match[4]);
    // A block that crosses midnight is written to the day it started on.
    minutes += end >= start ? end - start : 24 * 60 - start + end;
  }
  return { blocks, hours: minutes / 60 };
}

function todayIso(): string {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
}

function shift(date: string, days: number): string {
  const [y, m, d] = date.split("-").map(Number);
  const next = new Date(y, m - 1, d + days);
  const month = String(next.getMonth() + 1).padStart(2, "0");
  const day = String(next.getDate()).padStart(2, "0");
  return `${next.getFullYear()}-${month}-${day}`;
}

export function DayView({ date }: { date?: string } = {}) {
  const [selected, setSelected] = useState(todayIso);
  const [month, setMonth] = useState(() => {
    const now = new Date();
    return { year: now.getFullYear(), month: now.getMonth() + 1 };
  });
  const [days, setDays] = useState<DayEntry[]>([]);
  const [dayMarkdown, setDayMarkdown] = useState<string | null>(null);
  const [summaryMarkdown, setSummaryMarkdown] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<Outcome | null>(null);
  const [hasEngine, setHasEngine] = useState(false);
  const [job, setJob] = useState<JobState | null>(null);
  // A run you started yourself, and the message it failed with. The
  // scheduler's last outcome is a different fact about a possibly different
  // day, and must never stand in for this one.
  const [manualFailure, setManualFailure] = useState<{
    date: string;
    message: string;
    when: string;
  } | null>(null);
  // Raw is the default for today: today is what you are still recording.
  const [mode, setMode] = useState<"raw" | "summary">(() =>
    selected === todayIso() ? "raw" : "summary",
  );

  // Selecting a day always brings the calendar with it, so the rail and the
  // pane never disagree about which day you are looking at.
  const selectDate = useCallback((date: string) => {
    setSelected(date);
    const [year, month] = date.split("-").map(Number);
    if (year && month) setMonth({ year, month });
  }, []);

  // The window can be opened for a particular day, by the tray or by an
  // agent over MCP. Taken once on mount, and listened for while open.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const pending = await invoke<string | null>("take_pending_day");
        if (!cancelled && pending) selectDate(pending);
      } catch {
        // An older build without the command: today is the right answer.
      }
    })();
    const unlisten = listen<string>("open-day", (event) => {
      if (event.payload) selectDate(event.payload);
    });
    return () => {
      cancelled = true;
      void unlisten.then((off) => off()).catch(() => undefined);
    };
  }, [selectDate]);

  // The same effect the open-day event has, on an internal route: the
  // Overview map opens a day without going through Tauri. Routed through
  // selectDate, not setSelected directly, so the rail's month follows a
  // cross-month click instead of leaving the calendar on the month it
  // already had.
  useEffect(() => {
    if (date) selectDate(date);
  }, [date, selectDate]);

  const refreshMonth = useCallback(async () => {
    const entries = await invoke<DayEntry[]>("days_in_month", {
      year: month.year,
      month: month.month,
    });
    setDays(entries);
  }, [month.year, month.month]);

  useEffect(() => {
    void refreshMonth();
  }, [refreshMonth]);

  // The last completed run, read on its own schedule. It is deliberately not
  // a dependency of the day load: that is what made the two re-enter each
  // other once any run had recorded an outcome.
  const refreshOutcome = useCallback(async () => {
    const status = await invoke<Outcome | null>("job_status");
    setOutcome((current) => (sameOutcome(current, status) ? current : status));
    // A later run for the same day supersedes the manual failure; a run for
    // any other day leaves it alone.
    setManualFailure((current) =>
      current && status && status.date === current.date && status.when > current.when
        ? null
        : current,
    );
  }, []);

  useEffect(() => {
    void refreshOutcome();
  }, [refreshOutcome]);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const [day, summary, settings] = await Promise.all([
        invoke<string | null>("read_day", { date: selected }),
        invoke<string | null>("read_summary", { date: selected }),
        invoke<{ engine: unknown }>("get_settings"),
      ]);
      if (cancelled) return;
      setDayMarkdown(day);
      setSummaryMarkdown(summary);
      setHasEngine(settings.engine !== null);
      // Raw is the default for today; Summary for a past day that has one.
      setMode((current) =>
        selected === todayIso() ? current : summary ? "summary" : "raw",
      );
    })();
    return () => {
      cancelled = true;
    };
  }, [selected]);

  // Today's file grows while you look at it; refresh it live.
  useEffect(() => {
    if (selected !== todayIso()) return;
    const id = setInterval(async () => {
      const day = await invoke<string | null>("read_day", { date: selected });
      setDayMarkdown(day);
      void refreshOutcome();
    }, 5000);
    return () => clearInterval(id);
  }, [selected, refreshOutcome]);

  const onPrev = useCallback(
    () => selectDate(shift(selected, -1)),
    [selectDate, selected],
  );
  const onNext = useCallback(
    () => selectDate(shift(selected, 1)),
    [selectDate, selected],
  );
  const onToday = useCallback(() => selectDate(todayIso()), [selectDate]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.target instanceof HTMLInputElement) return;
      if (event.key === "ArrowLeft") onPrev();
      if (event.key === "ArrowRight") onNext();
      if (event.key.toLowerCase() === "t") onToday();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onPrev, onNext, onToday]);

  const reloadDay = useCallback(async () => {
    const [day, summary] = await Promise.all([
      invoke<string | null>("read_day", { date: selected }),
      invoke<string | null>("read_summary", { date: selected }),
    ]);
    setDayMarkdown(day);
    setSummaryMarkdown(summary);
    await refreshOutcome();
    void refreshMonth();
  }, [selected, refreshMonth, refreshOutcome]);

  // Runs are queued and serial. The command returns a job id straight away;
  // the view follows that one job and nobody else's.
  const onSummarise = useCallback(async () => {
    setMode("summary");
    try {
      const started = await invoke<{ job_id: string }>("summarise_now", {
        date: selected,
      });
      setJob({ id: started.job_id, date: selected, status: "queued", stderr: null });
      setManualFailure((current) =>
        current && current.date === selected ? null : current,
      );
    } catch (error) {
      setJob({
        id: "",
        date: selected,
        status: "failed",
        stderr: String(error),
      });
      setManualFailure({
        date: selected,
        message: String(error),
        when: new Date().toISOString(),
      });
    }
  }, [selected]);

  const jobId = job && job.date === selected ? job.id : null;
  const jobStatus = job && job.date === selected ? job.status : null;
  const pending = jobStatus === "queued" || jobStatus === "running";

  useEffect(() => {
    if (!jobId || !pending) return;
    let cancelled = false;
    const id = setInterval(() => {
      void (async () => {
        const state = await invoke<JobState | null>("job_state", { jobId });
        if (cancelled || !state) return;
        setJob((current) =>
          current && current.id === state.id && current.status === state.status
            ? current
            : { id: state.id, date: state.date, status: state.status, stderr: state.stderr },
        );
        if (state.status === "failed") {
          setManualFailure({
            date: state.date,
            message: state.stderr ?? "The run failed.",
            when: new Date().toISOString(),
          });
        }
        if (state.status === "done") {
          setManualFailure((current) =>
            current && current.date === state.date ? null : current,
          );
          await reloadDay();
        }
      })();
    }, 2000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [jobId, pending, reloadDay]);

  const entry = useMemo(
    () => days.find((d) => d.date === selected) ?? null,
    [days, selected],
  );
  const stats = useMemo(() => dayStats(dayMarkdown), [dayMarkdown]);

  const running = pending;

  const summary: SummaryState = useMemo(() => {
    if (jobStatus === "queued") return { kind: "queued" };
    if (jobStatus === "running") return { kind: "running" };
    if (manualFailure && manualFailure.date === selected) {
      return { kind: "failed", message: manualFailure.message };
    }
    if (summaryMarkdown) {
      const at = outcome && outcome.date === selected ? outcome.when : "";
      return { kind: "generated", at };
    }
    if (outcome && outcome.date === selected && !outcome.ok) {
      return { kind: "failed", message: outcome.message };
    }
    return { kind: "none" };
  }, [summaryMarkdown, outcome, selected, jobStatus, manualFailure]);

  const onMonthChange = useCallback((year: number, month: number) => {
    setMonth({ year, month });
  }, []);

  return (
    <div className="day-view">
      <CalendarRail
        year={month.year}
        month={month.month}
        days={days}
        selected={selected}
        onSelect={selectDate}
        onMonthChange={onMonthChange}
      />
      <div className="day-main">
        <DayHeader
          date={selected}
          entry={entry}
          stats={stats}
          summary={summary}
          mode={mode}
          onMode={setMode}
          onPrev={onPrev}
          onNext={onNext}
          onToday={onToday}
          onSummarise={onSummarise}
        />
        {mode === "summary" ? (
          <SummaryPane
            markdown={summaryMarkdown}
            hasCapture={entry?.has_capture ?? false}
            hasEngine={hasEngine}
            running={running}
            onSummarise={onSummarise}
            date={selected}
          />
        ) : (
          <RawPane date={selected} mode={mode} />
        )}
      </div>
    </div>
  );
}
