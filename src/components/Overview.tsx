import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { DefragControls } from "./DefragControls";
import { DefragMap } from "./DefragMap";
import { EyePanel } from "./EyePanel";
import { RecentNotes } from "./RecentNotes";
import { GITHUB_NEW_ISSUE, GITHUB_REPO } from "../lib/github";
import { useDefragState } from "../lib/useDefragState";
import type { AppStatus } from "../lib/status";
import type { Settings } from "../lib/days";

function open(url: string) {
  void invoke("open_link", { url });
}

/// Preview on the left, record and notes on the right: the Display
/// Properties shape the CRT already borrows. The Status group that used to
/// sit here said what the status bar along the bottom of the window already
/// says.
export function Overview({
  status,
  onOpenDay,
  onOpenAgent,
}: {
  status: AppStatus;
  onOpenDay: (date: string) => void;
  onOpenAgent: () => void;
}) {
  const { capture, setCapture, ready } = status;
  const defrag = useDefragState();
  const [hasAgent, setHasAgent] = useState(false);

  useEffect(() => {
    void invoke<Settings>("get_settings").then((saved) =>
      setHasAgent(saved.agent !== null),
    );
  }, []);

  return (
    <div className="overview">
      <div className="overview-preview">
        <EyePanel capture={capture} ready={ready} onCapture={setCapture} />
        <div className="overview-links">
          <button type="button" onClick={() => open(GITHUB_REPO)}>
            Star on GitHub
          </button>
          <button type="button" onClick={() => open(GITHUB_NEW_ISSUE)}>
            Report a bug
          </button>
        </div>
      </div>

      <div className="overview-side">
        {hasAgent ? null : (
          <fieldset>
            <legend>Agent</legend>
            <p className="agent-missing">
              No agent connected. Processing a day needs one.{" "}
              <button type="button" onClick={onOpenAgent}>
                Set up an agent
              </button>
            </p>
          </fieldset>
        )}

        <fieldset className="overview-record">
          <legend>Record</legend>
          <DefragMap
            days={defrag.days}
            failed={defrag.failed}
            today={defrag.today}
            active={defrag.active}
            onOpenDay={onOpenDay}
          />
          <DefragControls
            pending={defrag.pending}
            running={defrag.running}
            finished={defrag.finished}
            total={defrag.total}
            status={defrag.status}
            hasAgent={hasAgent}
            onStart={() => void defrag.start()}
            onStop={() => void defrag.stop()}
          />
          {ready ? null : (
            <div className="button-row">
              <button type="button" onClick={() => void invoke("open_setup")}>
                Finish setup…
              </button>
            </div>
          )}
        </fieldset>

        <RecentNotes
          days={defrag.days}
          hasAgent={hasAgent}
          onOpenDay={onOpenDay}
        />
      </div>
    </div>
  );
}
