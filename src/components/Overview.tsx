import { invoke } from "@tauri-apps/api/core";
import { EyePanel } from "./EyePanel";
import type { AppStatus } from "../lib/status";

/// The first tab: what the app is doing right now, and the two facts that
/// decide whether it can do it at all. Anything that needs changing rather
/// than reading lives in Settings.
export function Overview({ status }: { status: AppStatus }) {
  const { capture, setCapture, permission, folder, ready } = status;

  return (
    <div className="overview">
      <EyePanel capture={capture} ready={ready} onCapture={setCapture} />

      <fieldset>
        <legend>Status</legend>
        <p className="status-line">
          <span
            className={capture.running ? "led led-on" : "led"}
            aria-hidden="true"
          />
          {capture.blocks_today === 1
            ? "1 block written today"
            : `${capture.blocks_today} blocks written today`}
        </p>
        {permission === "granted" ? (
          <p className="status-line done">
            <span className="led led-on" aria-hidden="true" /> Access allowed.
          </p>
        ) : (
          <p className="status-line">
            <span className="led" aria-hidden="true" /> macOS has not granted
            access, so nothing can be read.
          </p>
        )}
        {folder ? (
          <p className="status-line done">
            <span className="led led-on" aria-hidden="true" /> Saving to{" "}
            {folder}
          </p>
        ) : (
          <p className="status-line">
            <span className="led" aria-hidden="true" /> No folder chosen yet.
          </p>
        )}
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
