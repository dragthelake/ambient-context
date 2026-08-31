import { useState } from "react";
import type { DayEntry } from "./CalendarRail";
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
  onOpenInEditor: () => void;
  onReveal: () => void;
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
  onOpenInEditor,
  onReveal,
}: DayHeaderProps) {
  const [showStderr, setShowStderr] = useState(false);
  const hasSummary = summary.kind === "generated";

  const summaryLine = (() => {
    switch (summary.kind) {
      case "none":
        return "No summary yet";
      case "running":
        return "Summarising now";
      case "generated":
        return summary.at ? `Generated at ${summary.at.slice(11, 16)}` : "Generated";
      case "failed":
        return "Last run failed";
    }
  })();

  return (
    <header className="day-header">
      <div className="day-nav">
        <button type="button" onClick={onPrev} aria-label="Previous day">
          ◀
        </button>
        <h1 className="day-date">{longDate(date)}</h1>
        <button type="button" onClick={onNext} aria-label="Next day">
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
            <>
              {" "}
              <button
                type="button"
                className="link-button"
                onClick={() => setShowStderr((shown) => !shown)}
              >
                {showStderr ? "Hide detail" : "Show detail"}
              </button>
              {showStderr ? (
                <pre className="day-error">{summary.message}</pre>
              ) : null}
            </>
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
            disabled
            title="Coming in 0.3"
            tabIndex={-1}
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
        <button type="button" onClick={onSummarise}>
          {hasSummary ? "Regenerate" : "Summarise"}
        </button>
        <button type="button" onClick={onOpenInEditor}>
          Open in editor
        </button>
        <button type="button" onClick={onReveal}>
          Reveal in Finder
        </button>
      </div>
      {entry?.title ? <p className="day-title">{entry.title}</p> : null}
    </header>
  );
}
