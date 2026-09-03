import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ProposalView } from "./HighlightPill";
import type { Proposal, ProposeError, ProposeTarget, Selection } from "../lib/rules";

type ProposePopoverProps = {
  target: ProposeTarget;
  selection: Selection;
  hasAgent: boolean;
  onClose: () => void;
  onApplied?: () => void;
};

type PopoverState =
  | { status: "editing" }
  | { status: "running" }
  | { status: "proposed"; proposal: Proposal }
  | { status: "failed"; error: ProposeError };

const TARGET_LABEL: Record<ProposeTarget, string> = {
  rules: "capture rules",
  prompt: "summary prompt",
};

export function ProposePopover({
  target,
  selection,
  hasAgent,
  onClose,
  onApplied,
}: ProposePopoverProps) {
  const [instruction, setInstruction] = useState("");
  const [agentName, setAgentName] = useState<string>("");
  const [state, setState] = useState<PopoverState>({ status: "editing" });
  const [showRaw, setShowRaw] = useState(false);

  useEffect(() => {
    void invoke<{ agent: { label: string } | null }>("get_settings").then(
      (settings) => setAgentName(settings.agent?.label ?? "no agent"),
    );
  }, []);

  if (!hasAgent) {
    return (
      <div className="propose-popover">
        <p className="empty-state">
          No agent is connected. Connect one on the Agent tab to use this.
        </p>
        <button type="button" onClick={onClose}>
          Close
        </button>
      </div>
    );
  }

  const run = async () => {
    setState({ status: "running" });
    try {
      const proposal = await invoke<Proposal>("propose", {
        target,
        selection,
        instruction,
      });
      setState({ status: "proposed", proposal });
    } catch (error) {
      setState({ status: "failed", error: error as ProposeError });
    }
  };

  return (
    <div className="propose-popover">
      <h3 className="propose-title">Update {TARGET_LABEL[target]}</h3>
      <blockquote className="propose-quote">{selection.text.trim()}</blockquote>

      {state.status === "proposed" ? (
        <ProposalView
          proposal={state.proposal}
          onClose={onClose}
          onApplied={onApplied}
        />
      ) : (
        <>
          <label className="propose-field">
            What should change?
            <textarea
              value={instruction}
              onChange={(event) => setInstruction(event.target.value)}
              rows={3}
              placeholder={
                target === "rules"
                  ? "Never record this site again"
                  : "Keep the summaries shorter"
              }
            />
          </label>
          <p className="propose-agent">Using {agentName || "the connected agent"}.</p>

          {state.status === "running" ? (
            <p className="empty-state is-running">
              Running. The agent is rewriting the file; this can take a few
              minutes.
              <span className="blink" aria-hidden="true">
                _
              </span>
            </p>
          ) : null}

          {state.status === "failed" ? (
            <div className="propose-failure">
              <p className="warn">{state.error.kind === "agent_failed"
                ? state.error.stderr
                : state.error.kind === "invalid"
                  ? state.error.reason
                  : "No agent is connected."}</p>
              {state.error.kind === "invalid" ? (
                <>
                  <button
                    type="button"
                    className="link-button"
                    onClick={() => setShowRaw((shown) => !shown)}
                  >
                    {showRaw ? "Hide agent output" : "Show agent output"}
                  </button>
                  {showRaw ? <pre className="day-error">{state.error.raw}</pre> : null}
                </>
              ) : null}
            </div>
          ) : null}

          <div className="button-row">
            <button
              type="button"
              disabled={state.status === "running" || instruction.trim() === ""}
              onClick={() => void run()}
            >
              {state.status === "running" ? "Running…" : "Run"}
            </button>
            <button
              type="button"
              disabled={state.status === "running"}
              onClick={onClose}
            >
              Cancel
            </button>
          </div>
          <p className="propose-note">
            Nothing is written until you read the diff and press Apply.
          </p>
        </>
      )}
    </div>
  );
}
