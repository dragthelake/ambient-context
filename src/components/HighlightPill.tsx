import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
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
  | { status: "open" }
  | { status: "popover"; target: ProposeTarget };

export type Box = { left: number; top: number; width: number; height: number };

const MARGIN = 8;

/// Where the pill goes for a selection at `anchor`, kept inside `bounds`
/// (the visible part of the pane). Centred over the selection and above
/// it; below it when there is no room above; and never past either edge,
/// which is what clamping the centre point instead of the edges got
/// wrong. Null when the selection has scrolled out of the visible pane,
/// since a pill with nothing under it is a pill pointing at nothing.
export function placePill(
  anchor: Box,
  pill: { width: number; height: number },
  bounds: Box,
): { left: number; top: number } | null {
  const anchorBottom = anchor.top + anchor.height;
  const boundsBottom = bounds.top + bounds.height;
  if (anchorBottom < bounds.top || anchor.top > boundsBottom) return null;
  const minLeft = bounds.left + MARGIN;
  const maxLeft = bounds.left + bounds.width - pill.width - MARGIN;
  const centred = anchor.left + anchor.width / 2 - pill.width / 2;
  const left = Math.min(Math.max(centred, minLeft), Math.max(minLeft, maxLeft));
  const above = anchor.top - pill.height - MARGIN;
  const top = above >= bounds.top + MARGIN ? above : anchorBottom + MARGIN;
  return { left, top };
}

/// The visible part of the pane: its box cut to the viewport.
function visibleBounds(container: HTMLElement | null): Box {
  const view = { left: 0, top: 0, width: window.innerWidth, height: window.innerHeight };
  if (!container) return view;
  const rect = container.getBoundingClientRect();
  const left = Math.max(rect.left, 0);
  const top = Math.max(rect.top, 0);
  return {
    left,
    top,
    width: Math.min(rect.right, view.width) - left,
    height: Math.min(rect.bottom, view.height) - top,
  };
}

/// The current selection's box, read fresh each time so a scrolled pane
/// gives the new position rather than the one at selection time.
function selectionBox(): Box | null {
  const active = window.getSelection();
  if (!active || active.rangeCount === 0) return null;
  const rect = active.getRangeAt(0).getBoundingClientRect();
  return { left: rect.left, top: rect.top, width: rect.width, height: rect.height };
}

export function HighlightPill({
  container,
  buildSelection,
  hasAgent,
  onApplied,
}: HighlightPillProps) {
  const [state, setState] = useState<PillState>({ status: "closed" });
  const [selection, setSelection] = useState<Selection | null>(null);
  const [copied, setCopied] = useState(false);
  const [position, setPosition] = useState<{ left: number; top: number } | null>(null);
  const pillRef = useRef<HTMLDivElement | null>(null);
  // While the mouse is down the selection is still being made, and a pill
  // that follows every change of it jumps about under the pointer. It
  // opens on mouseup instead; a keyboard selection has no drag and opens
  // at once.
  const dragging = useRef(false);

  const openForSelection = useCallback(() => {
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
    if (dragging.current) return;
    setSelection(buildSelection());
    setState({ status: "open" });
  }, [container, buildSelection]);

  useEffect(() => {
    const onMouseDown = () => {
      dragging.current = true;
    };
    const onMouseUp = () => {
      dragging.current = false;
      openForSelection();
    };
    document.addEventListener("selectionchange", openForSelection);
    window.addEventListener("mousedown", onMouseDown);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      document.removeEventListener("selectionchange", openForSelection);
      window.removeEventListener("mousedown", onMouseDown);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, [openForSelection]);

  // Anchored to the selection as it is now, not as it was: the pane
  // scrolls under the pill and the window resizes, and both move the text.
  const place = useCallback(() => {
    const pill = pillRef.current;
    const anchor = selectionBox();
    if (!pill || !anchor) {
      setPosition(null);
      return;
    }
    setPosition(
      placePill(
        anchor,
        { width: pill.offsetWidth, height: pill.offsetHeight },
        visibleBounds(container),
      ),
    );
  }, [container]);

  useLayoutEffect(() => {
    if (state.status !== "open") return;
    place();
    // Capture phase, so a scroll anywhere under the window reaches here
    // without a listener on each scrolling element.
    window.addEventListener("scroll", place, true);
    window.addEventListener("resize", place);
    return () => {
      window.removeEventListener("scroll", place, true);
      window.removeEventListener("resize", place);
    };
  }, [state.status, place]);

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

  return (
    <div
      ref={pillRef}
      className="highlight-pill"
      style={{
        position: "fixed",
        left: position?.left ?? 0,
        top: position?.top ?? 0,
        // Unmeasured on its first paint, or pointing at text that has
        // scrolled out of the pane: present for measuring, not shown.
        visibility: position ? "visible" : "hidden",
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
