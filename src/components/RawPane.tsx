import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { domainOf, literalPattern, type RawBlock, type RulesPayload, type Selection } from "../lib/rules";
import type { DayFile, Settings } from "../lib/days";
import { HighlightPill } from "./HighlightPill";

type RawPaneProps = {
  date: string;
  mode: "raw" | "kb" | "summary";
  file: DayFile;
};

function todayIso(): string {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
}

export function RawPane({ date, mode, file }: RawPaneProps) {
  const [blocks, setBlocks] = useState<RawBlock[]>([]);
  const [rules, setRules] = useState<RulesPayload | null>(null);
  const [ruleError, setRuleError] = useState<string | null>(null);
  const [confirmed, setConfirmed] = useState<{ block: string; id: string } | null>(
    null,
  );
  const [selectionText, setSelectionText] = useState("");
  const [scrollY, setScrollY] = useState(0);
  const [hasAgent, setHasAgent] = useState(false);
  // Held in state, not a ref: the pill needs the element on the render it
  // is given, and assigning a ref does not schedule one.
  const [pane, setPane] = useState<HTMLElement | null>(null);

  const readRules = useCallback(async (): Promise<RulesPayload> => {
    const payload = await invoke<RulesPayload>("get_rules");
    setRules(payload);
    return payload;
  }, []);

  // The provenance the pill hands the agent: the block the highlighted
  // text came from, found through the block element the selection sits in.
  const buildSelection = useCallback((): Selection | null => {
    const active = window.getSelection();
    const text = active?.toString().trim();
    if (!text || !active || active.rangeCount === 0) return null;
    const anchor = active.anchorNode;
    const blockElement = anchor instanceof Element ? anchor : anchor?.parentElement;
    const block = blockElement?.closest(".raw-block");
    if (!block) return null;
    return {
      date,
      text,
      app: block.getAttribute("data-app"),
      title: block.getAttribute("data-title"),
      time_range: block.getAttribute("data-range"),
      mode: "raw",
    };
  }, [date]);

  const read = useCallback(async () => {
    const next = await invoke<RawBlock[]>("read_day_blocks", { date, file });
    setBlocks(next);
  }, [date, file]);

  useEffect(() => {
    let cancelled = false;
    void readRules().then((payload) => {
      if (payload && !cancelled) setRules(payload);
    });

    void read();
    if (date !== todayIso()) {
      return () => {
        cancelled = true;
      };
    }
    const id = setInterval(() => void read(), 5000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [date, file, read, readRules]);

  useEffect(() => {
    void invoke<Settings>("get_settings").then((settings) =>
      setHasAgent(settings.agent !== null),
    );
  }, []);

  // Whether there is anything to redact is a live question: follow the
  // selection rather than reading it during an unrelated render.
  useEffect(() => {
    const onChange = () =>
      setSelectionText(window.getSelection()?.toString() ?? "");
    onChange();
    document.addEventListener("selectionchange", onChange);
    return () => document.removeEventListener("selectionchange", onChange);
  }, []);

  // Hold the scroll position across a refresh: a view that jumps every
  // five seconds while you are reading it is worse than no refresh.
  const onScroll = useCallback((event: React.UIEvent<HTMLElement>) => {
    setScrollY(event.currentTarget.scrollTop);
  }, []);
  useEffect(() => {
    // The scroller, not the frame: the frame does not scroll.
    const pane = document.querySelector(".raw-pane-scroll");
    if (pane && mode === "raw") pane.scrollTop = scrollY;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [blocks]);

  const nextId = rules?.next_id ?? "r1";

  const addRule = useCallback(
    async (blockKey: string, rule: RawRuleInput) => {
      const before = new Set((rules?.rules ?? []).map((r) => r.id));
      try {
        const payload = await invoke<RulesPayload>("add_rule", { rule });
        setRules(payload);
        setRuleError(null);
        // The id the write actually used, read back from the list it wrote.
        // `next_id` has already moved on by the time the answer arrives.
        const added =
          payload.rules.find((r) => !before.has(r.id)) ??
          payload.rules[payload.rules.length - 1];
        if (added) setConfirmed({ block: blockKey, id: added.id });
        setTimeout(() => setConfirmed(null), 2500);
      } catch (error) {
        setRuleError(String(error));
      }
    },
    [rules],
  );

  const neverRecordApp = useCallback(
    async (blockKey: string, block: RawBlock) => {
      await addRule(blockKey, {
        id: nextId,
        target: { app: block.app },
        action: "exclude",
        note: `From ${date} ${block.start}`,
      });
    },
    [addRule, date, nextId],
  );

  const headingsOnlyForSite = useCallback(
    async (blockKey: string, block: RawBlock) => {
      const domain = block.url ? domainOf(block.url) : null;
      if (!domain) return;
      await addRule(blockKey, {
        id: nextId,
        target: { website: domain },
        action: "headings_only",
        note: `From ${date} ${block.start}`,
      });
    },
    [addRule, date, nextId],
  );

  const redactLikeThis = useCallback(
    async (blockKey: string, selected: string) => {
      const current = await invoke<Settings>("get_settings");
      await invoke("set_settings", {
        next: {
          ...current,
          extra_redaction_patterns: [
            ...current.extra_redaction_patterns,
            literalPattern(selected),
          ],
        },
      });
      setConfirmed({ block: blockKey, id: "redact" });
      setTimeout(() => setConfirmed(null), 2500);
    },
    [],
  );

  return (
    /* Frame and scroller are separate elements, as in the tab pane: a
       scroll container's padding sits inside its scrollable area and an
       inset bevel paints under its descendants, so with both jobs on one
       element the blocks ride over the frame at each end of a scroll. */
    <section className="raw-pane">
      <div
        className="raw-pane-scroll"
        onScroll={onScroll}
        ref={(element) => {
          setPane(element);
        }}
      >
      <HighlightPill
        container={pane}
        buildSelection={buildSelection}
        hasAgent={hasAgent}
        onApplied={() => void readRules()}
      />
      {ruleError ? <p className="warn">{ruleError}</p> : null}
      {blocks.length === 0 ? (
        <p className="empty-state">Nothing was recorded on this day.</p>
      ) : null}
      {blocks.map((block, index) => {
        // The position is part of the identity because time and app alone
        // are not unique: a poll every few seconds can close one block and
        // open another within the same minute in the same app. Blocks
        // arrive in chronological order and are replaced wholesale when
        // the day changes, so the index is stable for as long as the row
        // is on screen. This key is also what the confirmation is keyed
        // on, so a collision does not merely warn: it puts "Rule added"
        // on every twin at once.
        const key = `${index}-${block.start}-${block.app}`;
        return (
          <article
            key={key}
            className="raw-block"
            data-app={block.app}
            data-title={block.title}
            data-range={`${block.start}–${block.end}`}
          >
            <header className="raw-block-heading">
              <span className="raw-block-time">
                {block.start}–{block.end}
              </span>
              <span className="raw-block-app">{block.app}</span>
              {block.title ? (
                <span className="raw-block-title">{block.title}</span>
              ) : null}
            </header>
            {block.file ? <p className="raw-ref">file: {block.file}</p> : null}
            {block.url ? <p className="raw-ref">url: {block.url}</p> : null}
            {block.lines.length > 0 ? <BlockBody lines={block.lines} /> : null}
            {block.lines.length === 0 ? (
              <p className="raw-quiet" title="No text was recorded for this block: everything on screen had already been captured earlier today, or a rule keeps headings only.">
                No body
              </p>
            ) : null}
            <footer className="raw-actions">
              <button
                type="button"
                onClick={() => void neverRecordApp(key, block)}
                title={`Never record ${block.app}`}
              >
                {confirmed?.block === key && confirmed.id !== "redact"
                  ? "Rule added"
                  : "Never record this app"}
              </button>
              <button
                type="button"
                disabled={!block.url}
                title={
                  block.url
                    ? `Headings only for ${domainOf(block.url)}`
                    : "This block has no url reference, so there is no site to match"
                }
                onClick={() => void headingsOnlyForSite(key, block)}
              >
                Headings only for this site
              </button>
              <button
                type="button"
                disabled={selectionText.trim() === ""}
                title="Redact the selected text, exactly as written"
                onClick={() => {
                  if (selectionText.trim()) void redactLikeThis(key, selectionText);
                }}
              >
                {confirmed?.block === key && confirmed.id === "redact"
                  ? "Redaction added"
                  : "Redact text like this"}
              </button>
            </footer>
          </article>
        );
      })}
      </div>
    </section>
  );
}

type RawRuleInput = {
  id: string;
  target: { app: string } | { website: string } | { title: string };
  action: "exclude" | "headings_only" | "full";
  note?: string | null;
};

const COLLAPSED_LINES = 4;

function BlockBody({ lines }: { lines: string[] }) {
  const [expanded, setExpanded] = useState(false);
  if (expanded || lines.length <= COLLAPSED_LINES) {
    return (
      <div className="raw-body">
        {lines.map((line, index) => (
          <p key={index}>{line}</p>
        ))}
        {lines.length > COLLAPSED_LINES ? (
          <button type="button" className="link-button" onClick={() => setExpanded(false)}>
            Collapse
          </button>
        ) : null}
      </div>
    );
  }
  return (
    <div className="raw-body">
      {lines.slice(0, COLLAPSED_LINES).map((line, index) => (
        <p key={index}>{line}</p>
      ))}
      <button
        type="button"
        className="link-button"
        onClick={() => setExpanded(true)}
      >
        Show {lines.length - COLLAPSED_LINES} more lines
      </button>
    </div>
  );
}
