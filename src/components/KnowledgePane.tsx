import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { KNOWLEDGE_SECTIONS, type KnowledgeSection } from "../lib/days";

/// The calls that build the knowledge, in the order the line names them,
/// with the word the reader sees for each.
const CALLS = [
  ["ingest_messages", "messages"],
  ["ingest_apps", "apps"],
  ["ingest_websites", "websites"],
] as const;

/// What the backend writes for a file the agent had nothing for, and what
/// it writes for a file that was never produced. Both mean the same thing
/// to a reader, so both render as one muted line.
const NOTHING = ["", "Nothing evident.", "(not ingested)"];

/// `read_kb` with no file concatenates the six under `# name.md` headers.
/// Splitting them back apart here keeps the page to one call.
export function splitSections(text: string): Map<string, string> {
  const out = new Map<string, string>();
  let current: string | null = null;
  let body: string[] = [];
  for (const line of text.split("\n")) {
    const heading = /^# (\S+\.md)\s*$/.exec(line);
    if (heading) {
      if (current) out.set(current, body.join("\n").trim());
      current = heading[1];
      body = [];
      continue;
    }
    body.push(line);
  }
  if (current) out.set(current, body.join("\n").trim());
  return out;
}

export type Built = { at: string; sources: string[] };

/// The manifest's accepted calls and the latest time one of them ran. A
/// rejected call contributed nothing to the files, so it is not named.
export function readBuilt(manifest: string | null): Built | null {
  if (!manifest) return null;
  const disposition = new Map<string, string>();
  const at = new Map<string, string>();
  for (const line of manifest.split("\n")) {
    const match = /^(\w+)\.(disposition|at):\s*(.+)$/.exec(line.trim());
    if (!match) continue;
    (match[2] === "at" ? at : disposition).set(match[1], match[3].trim());
  }
  const sources: string[] = [];
  let latest = "";
  for (const [call, word] of CALLS) {
    if (disposition.get(call) !== "accepted") continue;
    sources.push(word);
    const when = at.get(call) ?? "";
    if (when > latest) latest = when;
  }
  if (sources.length === 0 || latest === "") return null;
  return { at: latest, sources };
}

function list(words: string[]): string {
  if (words.length <= 1) return words.join("");
  return `${words.slice(0, -1).join(", ")} and ${words[words.length - 1]}`;
}

/// Headings, task lines, bullets and paragraphs; the knowledge files use
/// nothing else.
function render(markdown: string): ReactNode[] {
  return markdown
    .split("\n")
    .filter((line) => line.trim() !== "")
    .map((line, index) => {
      if (line.startsWith("## ")) return <h3 key={index}>{line.slice(3)}</h3>;
      if (line.startsWith("- [ ] ") || line.startsWith("- [x] "))
        return (
          <p key={index} className="knowledge-task">
            {line.slice(6)}
          </p>
        );
      if (line.startsWith("- "))
        return (
          <p key={index} className="knowledge-item">
            {line.slice(2)}
          </p>
        );
      return <p key={index}>{line}</p>;
    });
}

export type KnowledgePaneProps = {
  date: string;
  section: KnowledgeSection;
  refreshKey: number;
  running: boolean;
  step: string | null;
  hasAgent: boolean;
  onGenerate: () => void;
};

function Pane({ children }: { children: ReactNode }) {
  return (
    <section className="knowledge-pane">
      <div className="knowledge-pane-scroll reading">{children}</div>
    </section>
  );
}

/// One section of the day's knowledge at a time, chosen by the strip in
/// the header, with the line saying when it was built above it.
export function KnowledgePane({
  date,
  section,
  refreshKey,
  running,
  step,
  hasAgent,
  onGenerate,
}: KnowledgePaneProps) {
  // `undefined` while the first read is in flight, `null` when the day has
  // no knowledge at all. The two states read very differently on the page.
  const [text, setText] = useState<string | null | undefined>(undefined);
  const [built, setBuilt] = useState<Built | null>(null);

  useEffect(() => {
    let cancelled = false;
    setText(undefined);
    void (async () => {
      const [knowledge, manifest] = await Promise.all([
        invoke<string | null>("read_kb", { date }),
        invoke<string | null>("read_kb", { date, file: "manifest.md" }),
      ]);
      if (cancelled) return;
      setText(knowledge);
      setBuilt(readBuilt(manifest));
    })();
    return () => {
      cancelled = true;
    };
  }, [date, refreshKey]);

  if (text === undefined) return <section className="knowledge-pane" />;

  if (running && text === null) {
    return (
      <Pane>
        <p className="empty-state is-running">
          {step ?? "Building the knowledge"}
          <span className="blink" aria-hidden="true">
            _
          </span>
        </p>
      </Pane>
    );
  }

  if (text === null) {
    return (
      <Pane>
        <p className="empty-state">Nothing built for this day yet.</p>
        <p className="empty-note">
          Generate reads the day's context and builds a structured wiki from
          it: people, commitments, threads, products, issues and reading.
        </p>
        {hasAgent ? (
          <button type="button" className="generate-now" onClick={onGenerate}>
            Generate
          </button>
        ) : (
          <p className="empty-note">
            Connect an agent on the Agent tab first.
          </p>
        )}
      </Pane>
    );
  }

  const sections = splitSections(text);
  const label = KNOWLEDGE_SECTIONS.find(([file]) => file === section)?.[1] ?? section;
  const body = sections.get(section) ?? "";

  return (
    <Pane>
      {built ? (
        <p className="knowledge-built">
          Built {built.at.slice(11, 16)} from {list(built.sources)}
        </p>
      ) : null}
      <section className="knowledge-section">
        <h2>{label}</h2>
        {NOTHING.includes(body) ? (
          <p className="empty-state">Nothing evident</p>
        ) : (
          render(body)
        )}
      </section>
    </Pane>
  );
}
