import { useState } from "react";

const LEGEND: { state: string; label: string }[] = [
  { state: "empty", label: "Nothing recorded" },
  { state: "raw", label: "Raw context" },
  { state: "summarised", label: "Summarised" },
  { state: "failed", label: "Failed" },
];

/// The reference's lower half: a status line, a segmented bar in a sunken
/// trough, a percentage, and a button row with two at each end.
export function DefragControls({
  pending,
  running,
  finished,
  total,
  status,
  hasEngine,
  onStart,
  onStop,
}: {
  pending: string[];
  running: boolean;
  finished: number;
  total: number;
  status: string;
  hasEngine: boolean;
  onStart: () => void;
  onStop: () => void;
}) {
  const [legend, setLegend] = useState(false);
  const percent = total === 0 ? 0 : Math.round((finished / total) * 100);
  const days = pending.length === 1 ? "1 day" : `${pending.length} days`;

  return (
    <div className="defrag-controls">
      <p className="defrag-status">{status}</p>

      {/* Segmented, as the period's bars were: whole blocks appear rather
          than a bar sliding, so progress is countable at a glance. */}
      <div
        className="defrag-bar"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={percent}
      >
        <div className="defrag-bar-fill" style={{ width: `${percent}%` }} />
      </div>

      <p className="defrag-percent">{percent}% Complete</p>

      {legend ? (
        <ul className="defrag-legend">
          {LEGEND.map((item) => (
            <li key={item.state}>
              <span className={`defrag-swatch is-${item.state}`} aria-hidden="true" />
              {item.label}
            </li>
          ))}
        </ul>
      ) : null}

      <div className="defrag-buttons">
        <button type="button" onClick={() => setLegend((on) => !on)}>
          Legend
        </button>
        <span className="defrag-spacer" />
        <button
          type="button"
          disabled={running || pending.length === 0 || !hasEngine}
          title={hasEngine ? undefined : "Connect an engine in Settings to use this."}
          onClick={onStart}
        >
          {`Summarise ${days}`}
        </button>
        <button type="button" disabled={!running} onClick={onStop}>
          Stop
        </button>
      </div>
    </div>
  );
}
