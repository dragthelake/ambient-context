import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { domainOf, literalPattern, type RawBlock, type RulesPayload } from "../lib/rules";
import type { Settings } from "../lib/days";

type RawPaneProps = {
  date: string;
  mode: "raw" | "summary";
};

function todayIso(): string {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
}

export function RawPane({ date, mode }: RawPaneProps) {
  const [blocks, setBlocks] = useState<RawBlock[]>([]);
  const [rules, setRules] = useState<RulesPayload | null>(null);
  const [ruleError, setRuleError] = useState<string | null>(null);
  const [confirmed, setConfirmed] = useState<string | null>(null);
  const [scrollY, setScrollY] = useState(0);

  const read = useCallback(async () => {
    const next = await invoke<RawBlock[]>("read_day_blocks", { date });
    setBlocks(next);
  }, [date]);

  useEffect(() => {
    let cancelled = false;
    void invoke<RulesPayload>("get_rules").then((payload) => {
      if (!cancelled) setRules(payload);
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
  }, [date, read]);

  // Hold the scroll position across a refresh: a view that jumps every
  // five seconds while you are reading it is worse than no refresh.
  const onScroll = useCallback((event: React.UIEvent<HTMLElement>) => {
    setScrollY(event.currentTarget.scrollTop);
  }, []);
  useEffect(() => {
    const pane = document.querySelector(".raw-pane");
    if (pane && mode === "raw") pane.scrollTop = scrollY;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [blocks]);

  const nextId = rules?.next_id ?? "r1";

  const addRule = useCallback(async (rule: RawRuleInput) => {
    try {
      const payload = await invoke<RulesPayload>("add_rule", { rule });
      setRules(payload);
      setRuleError(null);
      setConfirmed(rule.id);
      setTimeout(() => setConfirmed(null), 2500);
    } catch (error) {
      setRuleError(String(error));
    }
  }, []);

  const neverRecordApp = useCallback(
    async (block: RawBlock) => {
      await addRule({
        id: nextId,
        target: { app: block.app },
        action: "exclude",
        note: `From ${date} ${block.start}`,
      });
    },
    [addRule, date, nextId],
  );

  const headingsOnlyForSite = useCallback(
    async (block: RawBlock) => {
      const domain = block.url ? domainOf(block.url) : null;
      if (!domain) return;
      await addRule({
        id: nextId,
        target: { website: domain },
        action: "headings_only",
        note: `From ${date} ${block.start}`,
      });
    },
    [addRule, date, nextId],
  );

  const redactLikeThis = useCallback(
    async (selected: string) => {
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
      setConfirmed(`redact:${selected}`);
      setTimeout(() => setConfirmed(null), 2500);
    },
    [],
  );

  return (
    <section className="raw-pane" onScroll={onScroll}>
      {ruleError ? <p className="warn">{ruleError}</p> : null}
      {blocks.length === 0 ? (
        <p className="empty-state">Nothing was recorded on this day.</p>
      ) : null}
      {blocks.map((block) => {
        const key = `${block.start}-${block.app}`;
        return (
          <article key={key} className="raw-block">
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
                onClick={() => void neverRecordApp(block)}
                title={`Never record ${block.app}`}
              >
                {confirmed === nextId ? "Rule added" : "Never record this app"}
              </button>
              <button
                type="button"
                disabled={!block.url}
                title={
                  block.url
                    ? `Headings only for ${domainOf(block.url)}`
                    : "This block has no url reference, so there is no site to match"
                }
                onClick={() => void headingsOnlyForSite(block)}
              >
                Headings only for this site
              </button>
              <button
                type="button"
                disabled={window.getSelection()?.toString().length === 0}
                title="Redact the selected text, exactly as written"
                onClick={() => {
                  const selected = window.getSelection()?.toString();
                  if (selected) void redactLikeThis(selected);
                }}
              >
                Redact text like this
              </button>
            </footer>
          </article>
        );
      })}
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
