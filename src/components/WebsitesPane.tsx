import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { UrlTotal } from "../lib/days";

function minutes(secs: number): string {
  return `${Math.round(secs / 60)}m`;
}

export function WebsitesPane({ date }: { date: string }) {
  const [totals, setTotals] = useState<UrlTotal[] | null>(null);

  useEffect(() => {
    let cancelled = false;
    void invoke<UrlTotal[]>("website_totals", { date }).then((next) => {
      if (!cancelled) setTotals(next);
    });
    return () => {
      cancelled = true;
    };
  }, [date]);

  if (totals === null) return null;
  if (totals.length === 0) {
    return (
      <section className="websites-pane">
        <p className="pane-empty">No websites recorded.</p>
      </section>
    );
  }
  return (
    <section className="websites-pane">
      <div className="websites-pane-scroll">
        <table className="websites-table">
          <thead>
            <tr>
              <th>Domain</th>
              <th>Title</th>
              <th>Dwell</th>
              <th>Visits</th>
              <th>First</th>
              <th>Last</th>
            </tr>
          </thead>
          <tbody>
            {totals.map((row, index) => (
              <tr key={`${row.url}-${row.title}-${index}`} title={row.url}>
                <td>{row.domain || "(no url)"}</td>
                <td>
                  {row.url ? (
                    <a
                      href={row.url}
                      onClick={(event) => {
                        event.preventDefault();
                        void invoke("open_link", { url: row.url });
                      }}
                    >
                      {row.title}
                    </a>
                  ) : (
                    row.title
                  )}
                </td>
                <td className="num">{minutes(row.dwell_secs)}</td>
                <td className="num">{row.visits}</td>
                <td className="num">{row.first}</td>
                <td className="num">{row.last}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
