import { useCallback, useRef } from "react";
import { HighlightPill } from "./HighlightPill";
import type { ReactNode } from "react";
import type { Selection } from "../lib/rules";

export type SummaryPaneProps = {
  markdown: string | null;
  hasCapture: boolean;
  hasEngine: boolean;
  running: boolean;
  onSummarise: () => void;
  date: string;
};

/// Hides the leading frontmatter block, renders the headings, paragraphs
/// and lists the prompt produces, and passes anything else through as a
/// paragraph.
function render(markdown: string): ReactNode[] {
  const lines = markdown.split("\n");
  const out: ReactNode[] = [];
  // 0: frontmatter not started, 1: inside it, 2: finished with it.
  let frontmatter = lines[0]?.trim() === "---" ? 1 : 2;
  let list: string[] = [];

  const flushList = () => {
    if (list.length === 0) return;
    out.push(
      <ul key={`ul-${out.length}`}>
        {list.map((item, index) => (
          <li key={index}>{item}</li>
        ))}
      </ul>,
    );
    list = [];
  };

  lines.forEach((line, lineIndex) => {
    const trimmed = line.trim();
    if (frontmatter === 1) {
      if (lineIndex > 0 && trimmed === "---") frontmatter = 2;
      return;
    }
    if (trimmed.startsWith("# ")) {
      flushList();
      out.push(<h1 key={out.length}>{trimmed.slice(2)}</h1>);
    } else if (trimmed.startsWith("## ")) {
      flushList();
      out.push(<h2 key={out.length}>{trimmed.slice(3)}</h2>);
    } else if (trimmed.startsWith("- ")) {
      list.push(trimmed.slice(2));
    } else if (trimmed === "") {
      flushList();
    } else {
      flushList();
      out.push(<p key={out.length}>{trimmed}</p>);
    }
  });
  flushList();
  return out;
}

export function SummaryPane({
  markdown,
  hasCapture,
  hasEngine,
  running,
  onSummarise,
  date,
}: SummaryPaneProps) {
  const paneRef = useRef<HTMLElement | null>(null);

  const buildSelection = useCallback((): Selection | null => {
    const active = window.getSelection();
    const text = active?.toString().trim();
    if (!text) return null;
    return {
      date,
      text,
      app: null,
      title: null,
      time_range: null,
      mode: "summary",
    };
  }, [date]);

  if (!hasCapture) {
    return (
      <section className="summary-pane">
        <p className="empty-state">Nothing was recorded on this day.</p>
      </section>
    );
  }
  if (running) {
    return (
      <section className="summary-pane">
        <p className="empty-state is-running">
          Summarising now. The engine is reading the day file; this can take a
          few minutes.
          <span className="blink" aria-hidden="true">
            _
          </span>
        </p>
      </section>
    );
  }
  if (markdown) {
    return (
      <section
        className="summary-pane reading"
        ref={(element) => {
          paneRef.current = element;
        }}
      >
        <HighlightPill
          container={paneRef.current}
          buildSelection={buildSelection}
          hasEngine={hasEngine}
        />
        {render(markdown)}
      </section>
    );
  }
  if (!hasEngine) {
    return (
      <section className="summary-pane">
        <p className="empty-state">
          No summary yet, and no engine is connected. Connect one in Settings
          in the left rail, then come back and summarise.
        </p>
      </section>
    );
  }
  return (
    <section className="summary-pane">
      <p className="empty-state">
        No summary yet for this day. Summarise it from the button above.
      </p>
      <button type="button" className="summarise-now" onClick={onSummarise}>
        Summarise now
      </button>
    </section>
  );
}
