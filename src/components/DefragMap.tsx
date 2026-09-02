import { useLayoutEffect, useRef, useState } from "react";
import { buildCells, CELL_GAP, CELL_W, PITCH_W, type Cell } from "../lib/defrag";
import type { DayEntry } from "../lib/days";

const STATE_WORDS: Record<Cell["state"], string> = {
  empty: "Nothing recorded",
  raw: "Recorded, not processed",
  summarised: "Processed",
  failed: "Last run failed",
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
  active = null,
  onOpenDay,
}: {
  days: DayEntry[];
  failed: Set<string>;
  today: string;
  /** The date being summarised right now; its cell blinks while it works. */
  active?: string | null;
  onOpenDay: (date: string) => void;
}) {
  const field = useRef<HTMLDivElement>(null);
  const [columns, setColumns] = useState(1);
  const [hovered, setHovered] = useState<Cell | null>(null);

  // Measured, not assumed: the pane is resizable and the map is the only
  // thing on the tab whose shape depends on its own width. Measured on the
  // grid rather than the well, because the well's clientWidth includes the
  // padding that holds the grid inside its bevel, and columns counted from
  // that overflow the space the cells actually have.
  useLayoutEffect(() => {
    const node = field.current;
    if (!node) return;
    const measure = () =>
      // n columns occupy n boxes plus n-1 gaps, so the available width
      // gains one gap before dividing by the pitch.
      setColumns(
        Math.max(1, Math.floor((node.clientWidth + CELL_GAP) / PITCH_W)),
      );
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  const cells = buildCells(days, today, columns, failed);

  return (
    <div className="defrag-well">
      <div
        className="defrag-grid"
        ref={field}
        style={{ gridTemplateColumns: `repeat(${columns}, ${CELL_W}px)` }}
      >
        {cells.map((cell, index) => (
          <button
            key={cell.date || `pad-${index}`}
            type="button"
            className={`defrag-cell is-${cell.state}${
              active !== null && cell.date === active ? " is-working" : ""
            }`}
            disabled={cell.state === "empty"}
            aria-label={cell.date ? longDate(cell.date) : undefined}
            onMouseEnter={() => setHovered(cell)}
            onMouseLeave={() => setHovered(null)}
            onFocus={() => setHovered(cell)}
            onBlur={() => setHovered(null)}
            onClick={() => cell.entry && onOpenDay(cell.date)}
          />
        ))}
      </div>
      {days.length === 0 ? (
        <p className="defrag-empty">
          Nothing recorded yet. Each day you capture fills one cell.
        </p>
      ) : null}
      {hovered?.entry ? (
        <div className="defrag-info" role="tooltip">
          <strong>{longDate(hovered.date)}</strong>
          <span>{STATE_WORDS[hovered.state]}</span>
          {/* No size for a day whose raw context is gone: the file's
              absence reads as 0 B, which looks like an empty day rather
              than a summarised one. */}
          {hovered.entry.has_capture ? <span>{size(hovered.entry.bytes)}</span> : null}
          {hovered.entry.title ? <span>{hovered.entry.title}</span> : null}
          <span className="defrag-info-hint">Click to open in Context</span>
        </div>
      ) : null}
    </div>
  );
}
