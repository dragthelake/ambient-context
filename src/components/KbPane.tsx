import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";

export const KB_FILES = [
  "people.md",
  "commitments.md",
  "threads.md",
  "products.md",
  "issues.md",
  "reading.md",
] as const;
export type KbFile = (typeof KB_FILES)[number] | "manifest.md";

function label(file: KbFile): string {
  const stem = file.replace(/\.md$/, "");
  return stem.charAt(0).toUpperCase() + stem.slice(1);
}

function stripFrontmatter(text: string): string {
  if (!text.startsWith("---\n")) return text;
  const end = text.indexOf("\n---\n", 4);
  return end === -1 ? text : text.slice(end + 5).replace(/^\n+/, "");
}

/// Headings, task lines, bullets and paragraphs; the KB files use nothing
/// else.
function render(markdown: string): ReactNode[] {
  return markdown
    .split("\n")
    .filter((line) => line.trim() !== "")
    .map((line, index) => {
      if (line.startsWith("## "))
        return <h3 key={index}>{line.slice(3)}</h3>;
      if (line.startsWith("- [ ] ") || line.startsWith("- [x] "))
        return (
          <p key={index} className="kb-task">
            {line.slice(6)}
          </p>
        );
      if (line.startsWith("- "))
        return (
          <p key={index} className="kb-item">
            {line.slice(2)}
          </p>
        );
      return <p key={index}>{line}</p>;
    });
}

export function KbPane({ date, refreshKey }: { date: string; refreshKey: number }) {
  const [file, setFile] = useState<KbFile>("people.md");
  const [text, setText] = useState<string | null | undefined>(undefined);

  useEffect(() => {
    let cancelled = false;
    setText(undefined);
    void invoke<string | null>("read_kb", { date, file }).then((next) => {
      if (!cancelled) setText(next);
    });
    return () => {
      cancelled = true;
    };
  }, [date, file, refreshKey]);

  return (
    <section className="kb-pane">
      <div className="segmented" role="tablist">
        {[...KB_FILES, "manifest.md" as const].map((name) => (
          <button
            key={name}
            type="button"
            role="tab"
            aria-selected={file === name}
            className={file === name ? "segment is-current" : "segment"}
            onClick={() => setFile(name)}
          >
            {label(name)}
          </button>
        ))}
      </div>
      <div className="kb-pane-scroll">
        {text === undefined ? null : text === null ? (
          <p className="pane-empty">Not ingested yet.</p>
        ) : file === "manifest.md" ? (
          <pre>{text}</pre>
        ) : (
          render(stripFrontmatter(text))
        )}
      </div>
    </section>
  );
}
