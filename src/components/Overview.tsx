import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { DefragControls } from "./DefragControls";
import { DefragMap } from "./DefragMap";
import { EyePanel } from "./EyePanel";
import { useDefragState } from "../lib/useDefragState";
import type { AppStatus } from "../lib/status";
import type { Settings } from "../lib/days";

/// The eye, then the record. The Status group that used to sit here said
/// what the status bar along the bottom of the window already says.
export function Overview({
  status,
  onOpenDay,
}: {
  status: AppStatus;
  onOpenDay: (date: string) => void;
}) {
  const { capture, setCapture, ready } = status;
  const defrag = useDefragState();
  const [hasEngine, setHasEngine] = useState(false);

  useEffect(() => {
    void invoke<Settings>("get_settings").then((saved) =>
      setHasEngine(saved.engine !== null),
    );
  }, []);

  return (
    <div className="overview">
      <EyePanel capture={capture} ready={ready} onCapture={setCapture} />

      <fieldset>
        <legend>Record</legend>
        <DefragMap
          days={defrag.days}
          failed={defrag.failed}
          today={defrag.today}
          onOpenDay={onOpenDay}
        />
        <DefragControls
          pending={defrag.pending}
          running={defrag.running}
          finished={defrag.finished}
          total={defrag.total}
          status={defrag.status}
          hasEngine={hasEngine}
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
    </div>
  );
}
