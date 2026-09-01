import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { DiffView } from "./DiffView";
import { ProposePopover } from "./ProposePopover";
import type { Proposal, ProposeTarget, Selection } from "../lib/rules";

type HighlightPillProps = {
  /** The element whose selections this pill watches. */
  container: HTMLElement | null;
  /** Builds the Selection for the current window selection, or null when
   * the highlighted text cannot be attributed to a block. */
  buildSelection: () => Selection | null;
  hasAgent: boolean;
  onApplied?: () => void;
};

type PillState =
  | { status: "closed" }
  | { status: "open"; rect: DOMRect }
  | { status: "popover"; target: ProposeTarget };

export function HighlightPill({
  container,
  buildSelection,
  hasAgent,
  onApplied,
}: HighlightPillProps) {
  const [state, setState] = useState<PillState>({ status: "closed" });
  const [selection, setSelection] = useState<Selection | null>(null);
  const [copied, setCopied] = useState(false);

  const onSelectionChange = useCallback(() => {
    const active = window.getSelection();
    const text = active?.toString().trim() ?? "";
    if (!text || !active || active.rangeCount === 0) {
      setState((current) =>
        current.status === "open" ? { status: "closed" } : current,
      );
      return;
    }
    // Only react to selections inside our own container.
    const anchor = active.anchorNode;
    if (container && anchor && !container.contains(anchor)) return;
    const rect = active.getRangeAt(0).getBoundingClientRect();
    setSelection(buildSelection());
    setState({ status: "open", rect });
  }, [container, buildSelection]);

  useEffect(() => {
    document.addEventListener("selectionchange", onSelectionChange);
    return () => document.removeEventListener("selectionchange", onSelectionChange);
  }, [onSelectionChange]);

  useEffect(() => {
    if (state.status !== "open") return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setState({ status: "closed" });
    };
    const onClick = (event: MouseEvent) => {
      const target = event.target as HTMLElement;
      if (!target.closest(".highlight-pill")) setState({ status: "closed" });
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("mousedown", onClick);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("mousedown", onClick);
    };
  }, [state.status]);

  const copyAsContext = async () => {
    if (!selection) return;
    const block = await invoke<string>("copy_context", { selection });
    await writeText(block);
    setCopied(true);
    setTimeout(() => {
      setCopied(false);
      setState({ status: "closed" });
    }, 1200);
  };

  if (state.status === "closed" || !selection) return null;

  if (state.status === "popover") {
    return (
      <ProposePopover
        target={state.target}
        selection={selection}
        hasAgent={hasAgent}
        onClose={() => setState({ status: "closed" })}
        onApplied={onApplied}
      />
    );
  }

  const { rect } = state;

  return (
    <div
      className="highlight-pill"
      style={{
        position: "fixed",
        left: Math.max(8, rect.left + rect.width / 2),
        top: Math.max(8, rect.top - 44),
        transform: "translateX(-50%)",
      }}
    >
      <Verbs
        hasAgent={hasAgent}
        copied={copied}
        onRules={() => setState({ status: "popover", target: "rules" })}
        onPrompt={() => setState({ status: "popover", target: "prompt" })}
        onCopy={() => void copyAsContext()}
      />
    </div>
  );
}

export function Verbs({
  hasAgent,
  copied,
  onRules,
  onPrompt,
  onCopy,
}: {
  hasAgent: boolean;
  copied: boolean;
  onRules: () => void;
  onPrompt: () => void;
  onCopy: () => void;
}) {
  return (
    <>
      <button
        type="button"
        disabled={!hasAgent}
        title={hasAgent ? undefined : "Connect an agent on the Agent tab to use this."}
        onClick={onRules}
      >
        Update capture rules…
      </button>
      <button
        type="button"
        disabled={!hasAgent}
        title={hasAgent ? undefined : "Connect an agent on the Agent tab to use this."}
        onClick={onPrompt}
      >
        Update summary prompt…
      </button>
      <button type="button" onClick={onCopy}>
        {copied ? "Copied" : "Copy as context"}
      </button>
    </>
  );
}

export function ProposalView({
  proposal,
  onClose,
  onApplied,
}: {
  proposal: Proposal;
  onClose: () => void;
  onApplied?: () => void;
}) {
  return (
    <div className="propose-popover">
      <DiffView
        proposal={proposal}
        onClose={onClose}
        onApplied={async () => {
          // After Apply on a rules proposal, re-read the rules so the
          // Settings list and the Raw pane agree with the file immediately.
          onApplied?.();
          onClose();
        }}
      />
    </div>
  );
}
