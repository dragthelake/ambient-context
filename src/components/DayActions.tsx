import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DayFile } from "../lib/days";
import type { DayMode } from "./DayView";

export type DayActionsProps = {
  date: string;
  mode: DayMode;
  rawFile: DayFile;
  running: boolean;
  hasAgent: boolean;
  hasKnowledge: boolean;
  hasNotes: boolean;
  onProcess: (force: boolean) => void;
  onGenerateKnowledge: (force: boolean) => void;
};

/// What the first button says and does on each tab. Context runs the whole
/// pipeline; Knowledge stops after the wiki; Notes runs the pipeline too,
/// which builds the knowledge first when it is missing.
export function primaryAction(
  mode: DayMode,
  hasKnowledge: boolean,
  hasNotes: boolean,
): { label: string; title: string; force: boolean; knowledgeOnly: boolean } {
  switch (mode) {
    case "context":
      return hasNotes
        ? {
            label: "Reprocess day",
            title: "Rebuild the knowledge from the record and write the notes again.",
            force: true,
            knowledgeOnly: false,
          }
        : {
            label: "Process day",
            title: "Build the knowledge from the record, then write the notes.",
            force: false,
            knowledgeOnly: false,
          };
    case "knowledge":
      return hasKnowledge
        ? {
            label: "Regenerate",
            title: "Build the knowledge again from the record.",
            force: true,
            knowledgeOnly: true,
          }
        : {
            label: "Generate",
            title: "Build the knowledge from the record.",
            force: false,
            knowledgeOnly: true,
          };
    case "notes":
      return hasNotes
        ? {
            label: "Regenerate",
            title: "Write the notes again from the knowledge.",
            force: true,
            knowledgeOnly: false,
          }
        : {
            label: "Generate",
            title: "Write the notes from the knowledge, building it first if needed.",
            force: false,
            knowledgeOnly: false,
          };
  }
}

/// The row under the content box: the tab's own action first, then the
/// two that open files. Under the box rather than in the header so what a
/// button acts on is what sits above it.
export function DayActions({
  date,
  mode,
  rawFile,
  running,
  hasAgent,
  hasKnowledge,
  hasNotes,
  onProcess,
  onGenerateKnowledge,
}: DayActionsProps) {
  const [actionError, setActionError] = useState<string | null>(null);
  const action = primaryAction(mode, hasKnowledge, hasNotes);

  const onOpen = async () => {
    try {
      // "context" is not a file the backend knows: it is whichever of the
      // three day files the strip above is showing.
      await invoke("open_in_editor", {
        date,
        which: mode === "notes" ? "summary" : mode === "knowledge" ? "kb" : rawFile,
      });
      setActionError(null);
    } catch (error) {
      setActionError(String(error));
    }
  };

  const onRevealDay = async () => {
    try {
      await invoke("reveal_day", { date });
      setActionError(null);
    } catch (error) {
      setActionError(String(error));
    }
  };

  return (
    <footer className="day-actions">
      <div className="day-action-buttons">
        <button
          type="button"
          disabled={running || !hasAgent}
          title={hasAgent ? action.title : "Connect an agent on the Agent tab to use this."}
          onClick={() =>
            action.knowledgeOnly ? onGenerateKnowledge(action.force) : onProcess(action.force)
          }
        >
          {action.label}
        </button>
        <button type="button" onClick={() => void onOpen()}>
          Open in editor
        </button>
        <button type="button" onClick={() => void onRevealDay()}>
          Reveal in Finder
        </button>
      </div>
      {actionError ? <p className="day-action-error">{actionError}</p> : null}
    </footer>
  );
}
