import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ProposalView } from "./HighlightPill";
import type { Proposal, ProposeError, ProposeTarget, Selection } from "../lib/rules";

type ProposePopoverProps = {
  target: ProposeTarget;
  selection: Selection;
  hasEngine: boolean;
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
  hasEngine,
  onClose,
  onApplied,
}: ProposePopoverProps) {
  const [instruction, setInstruction] = useState("");
  const [engineName, setEngineName] = useState<string>("");
  const [state, setState] = useState<PopoverState>({ status: "editing" });
  const [showRaw, setShowRaw] = useState(false);

  useEffect(() => {
    void invoke<{ engine: { label: string } | null }>("get_settings").then(
      (settings) => setEngineName(settings.engine?.label ?? "no engine"),
    );
  }, []);

  if (!hasEngine) {
    return (
      <div className="propose-popover">
        <p className="empty-state">
          No engine is connected. Connect one in Settings to use this.
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
          <p className="propose-engine">Using {engineName || "the connected engine"}.</p>

          {state.status === "running" ? (
            <p className="empty-state is-running">
              Running. The engine is rewriting the file; this can take a few
              minutes.
              <span className="blink" aria-hidden="true">
                _
              </span>
            </p>
          ) : null}

          {state.status === "failed" ? (
            <div className="propose-failure">
              <p className="warn">{state.error.kind === "engine_failed"
                ? state.error.stderr
                : state.error.kind === "invalid"
                  ? state.error.reason
                  : "No engine is connected."}</p>
              {state.error.kind === "invalid" ? (
                <>
                  <button
                    type="button"
                    className="link-button"
                    onClick={() => setShowRaw((shown) => !shown)}
                  >
                    {showRaw ? "Hide engine output" : "Show engine output"}
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
