import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
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

export type SummaryState =
  | { kind: "none" }
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

export function DayView() {
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
  const [running, setRunning] = useState(false);
  const [mode, setMode] = useState<"raw" | "summary">("summary");

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

  const onPrev = useCallback(() => setSelected((d) => shift(d, -1)), []);
  const onNext = useCallback(() => setSelected((d) => shift(d, 1)), []);
  const onToday = useCallback(() => {
    const today = todayIso();
    setSelected(today);
    const now = new Date();
    setMonth({ year: now.getFullYear(), month: now.getMonth() + 1 });
  }, []);

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

  const onSummarise = useCallback(async () => {
    setMode("summary");
    setRunning(true);
    try {
      await invoke("summarise_now", { date: selected });
    } catch (error) {
      setOutcome({ when: new Date().toISOString(), date: selected, ok: false, message: String(error) });
    }
    const summary = await invoke<string | null>("read_summary", { date: selected });
    setSummaryMarkdown(summary);
    await refreshOutcome();
    setRunning(false);
    void refreshMonth();
  }, [selected, refreshMonth, refreshOutcome]);

  const entry = useMemo(
    () => days.find((d) => d.date === selected) ?? null,
    [days, selected],
  );
  const stats = useMemo(() => dayStats(dayMarkdown), [dayMarkdown]);

  const summary: SummaryState = useMemo(() => {
    if (running) {
      return { kind: "running" };
    }
    if (summaryMarkdown) {
      const at = outcome && outcome.date === selected ? outcome.when : "";
      return { kind: "generated", at };
    }
    if (outcome && outcome.date === selected && !outcome.ok) {
      return { kind: "failed", message: outcome.message };
    }
    return { kind: "none" };
  }, [summaryMarkdown, outcome, selected, running]);

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
        onSelect={setSelected}
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
