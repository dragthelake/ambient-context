import { useCallback, useState } from "react";
import { HighlightPill } from "./HighlightPill";
import type { ReactNode } from "react";
import type { Selection } from "../lib/rules";

export type SummaryPaneProps = {
  markdown: string | null;
  hasCapture: boolean;
  hasAgent: boolean;
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

/// The pane's frame and its scrolling interior. Split because a scroll
/// container's padding lies inside its scrollable area and an inset bevel
/// paints under its descendants: with both on one element the content
/// rides over the frame at each end of a scroll. Every return below goes
/// through here so the five states cannot drift apart.
function Pane({
  reading,
  scrollRef,
  children,
}: {
  reading?: boolean;
  scrollRef?: (element: HTMLElement | null) => void;
  children: React.ReactNode;
}) {
  return (
    <section className={reading ? "summary-pane reading" : "summary-pane"}>
      <div className="summary-pane-scroll" ref={scrollRef}>
        {children}
      </div>
    </section>
  );
}

export function SummaryPane({
  markdown,
  hasCapture,
  hasAgent,
  running,
  onSummarise,
  date,
}: SummaryPaneProps) {
  // Held in state, not a ref: the pill needs the element on the render it
  // is given, and assigning a ref does not schedule one.
  const [pane, setPane] = useState<HTMLElement | null>(null);

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
      <Pane>
        <p className="empty-state">Nothing was recorded on this day.</p>
      </Pane>
    );
  }
  if (running) {
    return (
      <Pane>
        <p className="empty-state is-running">
          Summarising now. The agent is reading the day file; this can take a
          few minutes.
          <span className="blink" aria-hidden="true">
            _
          </span>
        </p>
      </Pane>
    );
  }
  if (markdown) {
    return (
      <Pane
        reading
        scrollRef={(element) => {
          setPane(element);
        }}
      >
        <HighlightPill
          container={pane}
          buildSelection={buildSelection}
          hasAgent={hasAgent}
        />
        {render(markdown)}
      </Pane>
    );
  }
  if (!hasAgent) {
    return (
      <Pane>
        <p className="empty-state">
          No summary yet, and no agent is connected. Connect one on the
          Agent tab, then come back and summarise.
        </p>
      </Pane>
    );
  }
  return (
    <Pane>
      <p className="empty-state">
        No summary yet for this day. Summarise it from the button above.
      </p>
      <button type="button" className="summarise-now" onClick={onSummarise}>
        Summarise now
      </button>
    </Pane>
  );
}
