import { invoke } from "@tauri-apps/api/core";
import { AsciiEye } from "./AsciiEye";
import { CrtMonitor } from "./CrtMonitor";
import { play, primeAudio } from "../lib/sound";
import type { CaptureStatus } from "../lib/status";

/// The eye, what it is doing, and the one control that changes it. Shared
/// by the Overview tab and the setup window so the two surfaces cannot
/// describe the same state differently. State is the caller's: it holds
/// the poll, and only it knows whether the app is set up enough to record.
export function EyePanel({
  capture,
  ready,
  onCapture,
}: {
  capture: CaptureStatus;
  ready: boolean;
  onCapture: (next: CaptureStatus) => void;
}) {
  return (
    <div className="eye-panel">
      <CrtMonitor glowing={capture.running}>
        <AsciiEye watching={capture.running} />
      </CrtMonitor>
      <p className="eye-caption">
        {capture.running
          ? "RECORDING"
          : ready
            ? "EYE CLOSED. NOTHING IS BEING RECORDED."
            : "SETUP REQUIRED. NOTHING IS BEING RECORDED."}
      </p>
      <button
        type="button"
        className="record-toggle"
        disabled={!ready && !capture.running}
        onClick={async () => {
          // Resume must start before the invoke: after await the browser
          // no longer treats playback as part of the click gesture.
          primeAudio();
          const next = await invoke<CaptureStatus>("toggle_capture");
          // Cue the state that was actually reached, not the one asked
          // for: the toggle refuses when permission or a folder is
          // missing, and a "started" chime would be a lie.
          play(next.running ? "ready" : "release");
          onCapture(next);
        }}
      >
        {capture.running ? "Stop recording" : "Start recording"}
      </button>
    </div>
  );
}
