import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DayEntry } from "../lib/days";
import type { DayStats, SummaryState } from "./DayView";

export type DayHeaderProps = {
  date: string;
  entry: DayEntry | null;
  stats: DayStats;
  summary: SummaryState;
  mode: "raw" | "summary";
  onMode: (mode: "raw" | "summary") => void;
  onPrev: () => void;
  onNext: () => void;
  onToday: () => void;
  onSummarise: () => void;
};

function longDate(date: string): string {
  const [y, m, d] = date.split("-").map(Number);
  return new Date(y, m - 1, d).toLocaleDateString("en-AU", {
    weekday: "long",
    day: "numeric",
    month: "long",
    year: "numeric",
  });
}

export function DayHeader({
  date,
  entry,
  stats,
  summary,
  mode,
  onMode,
  onPrev,
  onNext,
  onToday,
  onSummarise,
}: DayHeaderProps) {
  const [actionError, setActionError] = useState<string | null>(null);
  const hasSummary = summary.kind === "generated";

  const onOpen = async () => {
    try {
      // The raw view is the day file; "raw" is not a file the backend knows.
      await invoke("open_in_editor", {
        date,
        which: mode === "raw" ? "day" : "summary",
      });
      setActionError(null);
    } catch (error) {
      setActionError(String(error));
    }
  };

  const onRevealDay = async () => {
    try {
      await invoke("reveal_day", { date });
      setActionError(null);
    } catch (error) {
      setActionError(String(error));
    }
  };

  const summaryLine = (() => {
    switch (summary.kind) {
      case "none":
        return "No summary yet";
      case "queued":
        return "Queued";
      case "running":
        return "Summarising…";
      case "generated":
        return summary.at ? `Generated at ${summary.at.slice(11, 16)}` : "Generated";
      case "failed":
        return "Last run failed";
    }
  })();

  return (
    <header className="day-header">
      <div className="day-nav">
        <button
          type="button"
          className="day-nav-step"
          onClick={onPrev}
          aria-label="Previous day"
        >
          ◀
        </button>
        <h1 className="day-date">{longDate(date)}</h1>
        <button
          type="button"
          className="day-nav-step"
          onClick={onNext}
          aria-label="Next day"
        >
          ▶
        </button>
        <button type="button" className="day-today" onClick={onToday}>
          Today
        </button>
      </div>

      <div className="day-meta">
        <span className="day-stats">
          {stats.blocks} {stats.blocks === 1 ? "block" : "blocks"} ·{" "}
          {stats.hours.toFixed(1)} h
        </span>
        <span className={`day-summary-state ${summary.kind}`}>
          {summaryLine}
          {summary.kind === "failed" ? (
            <pre className="day-error">{summary.message}</pre>
          ) : null}
        </span>
      </div>

      <div className="day-actions">
        <div className="segmented" role="tablist">
          <button
            type="button"
            role="tab"
            aria-selected={mode === "raw"}
            className={mode === "raw" ? "segment is-current" : "segment"}
            onClick={() => onMode("raw")}
          >
            Raw
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={mode === "summary"}
            className={mode === "summary" ? "segment is-current" : "segment"}
            onClick={() => onMode("summary")}
          >
            Summary
          </button>
        </div>
        <div className="day-action-buttons">
          <button type="button" onClick={onSummarise}>
            {hasSummary ? "Regenerate" : "Summarise"}
          </button>
          <button type="button" onClick={() => void onOpen()}>
            Open in editor
          </button>
          <button type="button" onClick={() => void onRevealDay()}>
            Reveal in Finder
          </button>
        </div>
      </div>
      {actionError ? <p className="day-action-error">{actionError}</p> : null}
      {entry?.title ? <p className="day-title">{entry.title}</p> : null}
    </header>
  );
}
