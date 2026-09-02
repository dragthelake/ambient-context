import type { DayEntry } from "../lib/days";

const LIMIT = 12;

function shortDate(iso: string): string {
  const [y, m, d] = iso.split("-").map(Number);
  return new Date(y, m - 1, d).toLocaleDateString("en-AU", {
    weekday: "short",
    day: "numeric",
    month: "short",
  });
}

/// Newest first, only days that already have a written note. The map above
/// shows the whole record; this list is the way back into the days that
/// are worth opening.
export function processedDays(days: DayEntry[]): DayEntry[] {
  return days.filter((day) => day.has_summary).slice(0, LIMIT);
}

export type RecentNotesProps = {
  days: DayEntry[];
  hasAgent: boolean;
  onOpenDay: (date: string) => void;
};

export function RecentNotes({ days, hasAgent, onOpenDay }: RecentNotesProps) {
  const notes = processedDays(days);
  const recorded = days.some((day) => day.has_capture);

  return (
    <fieldset className="overview-notes">
      <legend>Notes</legend>
      {notes.length === 0 ? (
        <div className="overview-notes-empty">
          {!recorded ? (
            <>
              <p className="empty-state">Nothing recorded yet.</p>
              <p className="empty-note">
                Open the eye and the map will fill as you work. Process a day
                later to write its note.
              </p>
            </>
          ) : (
            <>
              <p className="empty-state">No notes yet.</p>
              <p className="empty-note">
                {hasAgent
                  ? "Days are on the map above. Process one to write a short note you can open from here."
                  : "Connect an agent, then process a day to write its note."}
              </p>
            </>
          )}
        </div>
      ) : (
        <ul className="overview-notes-list">
          {notes.map((day) => (
            <li key={day.date}>
              <button
                type="button"
                className="overview-note"
                onClick={() => onOpenDay(day.date)}
              >
                <span className="overview-note-date">{shortDate(day.date)}</span>
                <span className="overview-note-title">
                  {day.title?.trim() || "Untitled note"}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </fieldset>
  );
}
