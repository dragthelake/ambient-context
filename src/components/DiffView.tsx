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
  const apply = async () => {
    await invoke("apply_proposal", { id: proposal.id });
    await onApplied();
  };
  const discard = async () => {
    // Discard is not a cancel: it is a recorded decision.
    await invoke("discard_proposal", { id: proposal.id });
    onClose();
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
      <p className="propose-note">
        Nothing has been written yet. Apply writes the file; Discard records
        the decision and writes nothing.
      </p>
    </div>
  );
}
