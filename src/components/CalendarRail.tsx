import { useCallback } from "react";

export type DayEntry = {
  date: string;
  has_capture: boolean;
  has_summary: boolean;
  bytes: number;
  title: string | null;
};

export type CalendarRailProps = {
  year: number;
  month: number;
  days: DayEntry[];
  selected: string;
  onSelect: (date: string) => void;
  onMonthChange: (year: number, month: number) => void;
};

const WEEKDAYS = ["M", "T", "W", "T", "F", "S", "S"];
const MONTHS = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

function todayIso(): string {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
}

export function CalendarRail({
  year,
  month,
  days,
  selected,
  onSelect,
  onMonthChange,
}: CalendarRailProps) {
  const move = useCallback(
    (delta: number) => {
      const next = new Date(year, month - 1 + delta, 1);
      onMonthChange(next.getFullYear(), next.getMonth() + 1);
    },
    [year, month, onMonthChange],
  );

  // Monday-first grid, matching the weekday headings.
  const first = new Date(year, month - 1, 1);
  const leading = (first.getDay() + 6) % 7;
  const length = new Date(year, month, 0).getDate();

  const cells: (number | null)[] = [
    ...Array.from({ length: leading }, () => null),
    ...Array.from({ length }, (_, i) => i + 1),
  ];
  while (cells.length % 7 !== 0) cells.push(null);

  const monthPadded = String(month).padStart(2, "0");
  const today = todayIso();

  return (
    <nav className="calendar-rail">
      <div className="calendar-title">
        <button
          type="button"
          className="calendar-arrow"
          aria-label="Previous month"
          onClick={() => move(-1)}
        >
          ◀
        </button>
        <span className="calendar-month">
          {MONTHS[month - 1]} {year}
        </span>
        <button
          type="button"
          className="calendar-arrow"
          aria-label="Next month"
          onClick={() => move(1)}
        >
          ▶
        </button>
      </div>
      <div className="calendar-grid">
        {WEEKDAYS.map((weekday, index) => (
          <span key={index} className="calendar-weekday">
            {weekday}
          </span>
        ))}
        {cells.map((day, index) => {
          if (day === null) {
            return <span key={index} className="calendar-cell is-blank" />;
          }
          const iso = `${year}-${monthPadded}-${String(day).padStart(2, "0")}`;
          const entry = days.find((d) => d.date === iso);
          const classes = [
            "calendar-cell",
            iso === today ? "is-today" : "",
            iso === selected ? "is-selected" : "",
          ]
            .filter(Boolean)
            .join(" ");
          return (
            <button
              type="button"
              key={index}
              className={classes}
              onClick={() => onSelect(iso)}
            >
              <span className="calendar-daynum">{day}</span>
              {entry?.has_capture ? <span className="mark-capture" /> : null}
              {entry?.has_summary ? <span className="mark-summary" /> : null}
            </button>
          );
        })}
      </div>
      <p className="calendar-legend">
        <span className="mark-capture" /> captured
        <span className="mark-summary" /> summarised
      </p>
    </nav>
  );
}
