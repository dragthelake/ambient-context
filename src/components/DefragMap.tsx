import { useLayoutEffect, useRef, useState } from "react";
import { buildCells, CELL_W, type Cell } from "../lib/defrag";
import type { DayEntry } from "../lib/days";

const STATE_WORDS: Record<Cell["state"], string> = {
  empty: "Nothing recorded",
  raw: "Raw context, not summarised",
  summarised: "Summarised",
  failed: "Last summarise failed",
};

function longDate(iso: string): string {
  return new Date(`${iso}T00:00:00Z`).toLocaleDateString("en-AU", {
    day: "numeric",
    month: "long",
    year: "numeric",
    timeZone: "UTC",
  });
}

function size(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/// One cell per day in a Windows 98 Disk Defragmenter field. Columns come
/// from the well's own width rather than a constant, so the map reflows
/// with the window.
export function DefragMap({
  days,
  failed,
  today,
  onOpenDay,
}: {
  days: DayEntry[];
  failed: Set<string>;
  today: string;
  onOpenDay: (date: string) => void;
}) {
  const well = useRef<HTMLDivElement>(null);
  const [columns, setColumns] = useState(1);
  const [hovered, setHovered] = useState<Cell | null>(null);

  // Measured, not assumed: the pane is resizable and the map is the only
  // thing on the tab whose shape depends on its own width.
  useLayoutEffect(() => {
    const node = well.current;
    if (!node) return;
    const measure = () =>
      setColumns(Math.max(1, Math.floor(node.clientWidth / CELL_W)));
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  const cells = buildCells(days, today, columns, failed);

  return (
    <div className="defrag-well" ref={well}>
      <div
        className="defrag-grid"
        style={{ gridTemplateColumns: `repeat(${columns}, ${CELL_W}px)` }}
      >
        {cells.map((cell, index) => (
          <button
            key={cell.date || `pad-${index}`}
            type="button"
            className={`defrag-cell is-${cell.state}`}
            disabled={cell.entry === null}
            aria-label={cell.date ? longDate(cell.date) : undefined}
            onMouseEnter={() => setHovered(cell)}
            onMouseLeave={() => setHovered(null)}
            onFocus={() => setHovered(cell)}
            onBlur={() => setHovered(null)}
            onClick={() => cell.entry && onOpenDay(cell.date)}
          />
        ))}
      </div>
      {hovered?.entry ? (
        <div className="defrag-info" role="tooltip">
          <strong>{longDate(hovered.date)}</strong>
          <span>{STATE_WORDS[hovered.state]}</span>
          <span>{size(hovered.entry.bytes)}</span>
          {hovered.entry.title ? <span>{hovered.entry.title}</span> : null}
          <span className="defrag-info-hint">Click to open in Context</span>
        </div>
      ) : null}
    </div>
  );
}
