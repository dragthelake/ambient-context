import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Proposal } from "../lib/rules";

type DiffViewProps = {
  proposal: Proposal;
  onClose: () => void;
  onApplied: () => void | Promise<void>;
};

const TARGET_LABEL: Record<string, string> = {
  rules: "rules.json",
  prompt: "prompts/day-context.md",
};

export function DiffView({ proposal, onClose, onApplied }: DiffViewProps) {
  const [error, setError] = useState<string | null>(null);

  const apply = async () => {
    try {
      await invoke("apply_proposal", { id: proposal.id });
      setError(null);
      await onApplied();
    } catch (e) {
      setError(String(e));
    }
  };
  const discard = async () => {
    // Discard is not a cancel: it is a recorded decision.
    try {
      await invoke("discard_proposal", { id: proposal.id });
      setError(null);
      onClose();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="diff-view">
      <h4 className="diff-title">
        Proposed change to {TARGET_LABEL[proposal.target]}
      </h4>
      <p className="diff-reasoning">{proposal.reasoning}</p>
      <pre className="diff-body">
        {proposal.diff.split("\n").map((line, index) => (
          <span
            key={index}
            className={
              line.startsWith("+ ")
                ? "diff-added"
                : line.startsWith("- ")
                  ? "diff-removed"
                  : undefined
            }
          >
            {line}
            {"\n"}
          </span>
        ))}
      </pre>
      <div className="button-row">
        <button type="button" onClick={() => void discard()}>
          Discard
        </button>
        <button type="button" onClick={() => void apply()}>
          Apply
        </button>
      </div>
      {error ? <p className="warn">{error}</p> : null}
      <p className="propose-note">
        Nothing has been written yet. Apply writes the file; Discard records
        the decision and writes nothing.
      </p>
    </div>
  );
}
