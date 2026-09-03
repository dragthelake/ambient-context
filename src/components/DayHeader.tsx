import type { DayEntry, DayFile, KnowledgeSection } from "../lib/days";
import { KNOWLEDGE_SECTIONS } from "../lib/days";
import type { DayMode, DayStats, SummaryState } from "./DayView";

export type DayHeaderProps = {
  date: string;
  entry: DayEntry | null;
  stats: DayStats;
  summary: SummaryState;
  mode: DayMode;
  onMode: (mode: DayMode) => void;
  rawFile: DayFile;
  onRawFile: (file: DayFile) => void;
  section: KnowledgeSection;
  onSection: (section: KnowledgeSection) => void;
  onPrev: () => void;
  onNext: () => void;
  onToday: () => void;
  step: string | null;
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

/// How long the run took, in the coarsest unit that is still true. Rounding
/// a forty second run up to "1 min" would be a number the user can see is
/// wrong against the clock they watched it on.
export function took(ms: number): string {
  if (ms < 60_000) return `took ${Math.max(1, Math.round(ms / 1000))}s`;
  return `took ${Math.round(ms / 60_000)} min`;
}

/// The one line under the date. It says what was recorded and where the
/// notes are up to, and during a run it says which step is running: the
/// day has one state, so it gets one line.
export function statusLine(
  stats: DayStats,
  summary: SummaryState,
  step: string | null,
): string {
  if (summary.kind === "queued") return "Queued…";
  if (summary.kind === "running") return `${step ?? "Processing"}…`;
  const recorded = `${stats.hours.toFixed(1)} h recorded · ${stats.blocks} ${
    stats.blocks === 1 ? "block" : "blocks"
  }`;
  if (summary.kind === "failed") return `Last run failed: ${summary.message.split("\n")[0]}`;
  if (summary.kind === "none") return `${recorded} · No notes yet`;
  if (!summary.at) return `${recorded} · Notes written`;
  const when = `Notes ${summary.at.slice(11, 16)}`;
  return `${recorded} · ${when}${summary.tookMs ? `, ${took(summary.tookMs)}` : ""}`;
}

function Strip<K extends string>({
  items,
  current,
  onSelect,
}: {
  items: readonly (readonly [K, string])[];
  current: K;
  onSelect: (key: K) => void;
}) {
  return (
    <div className="segmented" role="tablist">
      {items.map(([key, label]) => (
        <button
          key={key}
          type="button"
          role="tab"
          aria-selected={current === key}
          className={current === key ? "segment is-current" : "segment"}
          onClick={() => onSelect(key)}
        >
          {label}
        </button>
      ))}
    </div>
  );
}

const MODES = [
  ["context", "Context"],
  ["knowledge", "Knowledge"],
  ["notes", "Notes"],
] as const;

const RAW_FILES = [
  ["apps", "Apps"],
  ["websites", "Websites"],
  ["messages", "Messages"],
] as const;

/// Navigation and the tab strips only. The actions live under the content
/// box in DayActions, so a strip appearing or going cannot move a button.
export function DayHeader({
  date,
  entry,
  stats,
  summary,
  mode,
  onMode,
  rawFile,
  onRawFile,
  section,
  onSection,
  onPrev,
  onNext,
  onToday,
  step,
}: DayHeaderProps) {
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
        <p className={`day-stats ${summary.kind}`}>{statusLine(stats, summary, step)}</p>
        {summary.kind === "failed" ? (
          <pre className="day-error">{summary.message}</pre>
        ) : null}
      </div>

      <div className="day-tabs">
        <Strip items={MODES} current={mode} onSelect={onMode} />
        {mode === "context" ? (
          <Strip items={RAW_FILES} current={rawFile} onSelect={onRawFile} />
        ) : null}
        {mode === "knowledge" ? (
          <Strip items={KNOWLEDGE_SECTIONS} current={section} onSelect={onSection} />
        ) : null}
      </div>
      {entry?.title ? <p className="day-title">{entry.title}</p> : null}
    </header>
  );
}
