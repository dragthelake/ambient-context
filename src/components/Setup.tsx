import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AsciiEye } from "./AsciiEye";

type Permission = "granted" | "notGranted";

type CaptureStatus = {
  running: boolean;
  blocks_today: number;
};

function looksSynced(folder: string): boolean {
  return (
    folder.includes("Mobile Documents") ||
    folder.includes("com~apple~CloudDocs") ||
    folder.includes("iCloud Drive")
  );
}

function closeWindow() {
  void getCurrentWindow().close();
}

export function Setup() {
  const [permission, setPermission] = useState<Permission>("notGranted");
  const [asked, setAsked] = useState(false);
  const [folder, setFolder] = useState<string | null>(null);
  const [picking, setPicking] = useState(false);
  const [capture, setCapture] = useState<CaptureStatus>({
    running: false,
    blocks_today: 0,
  });

  // The toggle lives in the menu bar, so this page only ever observes
  // capture state; a 1s poll keeps it honest without any event plumbing.
  useEffect(() => {
    let cancelled = false;
    const read = async () => {
      const next = await invoke<CaptureStatus>("capture_status");
      if (!cancelled) setCapture(next);
    };
    void read();
    const id = setInterval(read, 1000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, []);

  useEffect(() => {
    invoke<string | null>("current_folder").then((next) => setFolder(next ?? null));
    invoke<Permission>("permission_status").then(setPermission);
  }, []);

  useEffect(() => {
    if (permission === "granted") return;
    const id = setInterval(async () => {
      const next = await invoke<Permission>("permission_status");
      setPermission(next);
    }, 700);
    return () => clearInterval(id);
  }, [permission]);

  // Escape closes the window, as it would any dialog.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") closeWindow();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const pickFolder = async (command: "choose_folder" | "use_default_folder") => {
    if (picking) return;
    setPicking(true);
    try {
      const next = await invoke<string | null>(command);
      if (next) setFolder(next);
    } finally {
      setPicking(false);
    }
  };

  const ready = permission === "granted" && folder !== null;

  // Becoming ready mid-session (the grant arriving from System Settings)
  // starts recording the same way a ready launch would.
  useEffect(() => {
    if (!ready) return;
    invoke<CaptureStatus>("start_if_enabled").then(setCapture);
  }, [ready]);

  return (
    <main className="window">
      <div className="titlebar" data-tauri-drag-region>
        <span className="titlebar-text" data-tauri-drag-region>
          AMBIENT_CONTEXT
        </span>
        <button
          type="button"
          className="titlebar-button"
          aria-label="Close settings"
          onClick={closeWindow}
        >
          ×
        </button>
      </div>

      <div className="window-body">
        <div className="eye-panel">
          <AsciiEye watching={capture.running} />
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
            onClick={async () =>
              setCapture(await invoke<CaptureStatus>("toggle_capture"))
            }
          >
            {capture.running ? "Stop recording" : "Start recording"}
          </button>
        </div>

        <fieldset>
          <legend>What it does</legend>
          <p>
            Ambient Context reads the text of whichever window you have
            focused and writes it to a markdown file on your computer. One
            file per day, in a folder you choose.
          </p>
          <ul>
            <li>It never takes screenshots or records your screen.</li>
            <li>It only reads the window you are actually looking at.</li>
            <li>
              The app does not upload captured text. There is no account and
              no server.
            </li>
            <li>
              It skips secure password fields and discards snapshots matching
              its known password-manager and private-browser rules before
              writing.
            </li>
            <li>
              Stop it any time from the menu bar; it stays stopped until you
              start it again.
            </li>
          </ul>
        </fieldset>

        <fieldset>
          <legend>1. Allow access</legend>
          <p>
            macOS needs your permission to read another app's windows.
            Choosing Allow opens System Settings, where you switch Ambient
            Context on.
          </p>
          {permission === "granted" ? (
            <p className="status-line done">
              <span className="led led-on" aria-hidden="true" /> Access
              allowed.
            </p>
          ) : (
            <>
              <button
                type="button"
                onClick={() => {
                  setAsked(true);
                  void invoke("request_permission");
                }}
              >
                {asked ? "Ask again" : "Allow access"}
              </button>
              {asked ? (
                <p className="status-line waiting">
                  Waiting for you in System Settings
                  <span className="blink" aria-hidden="true">
                    _
                  </span>
                </p>
              ) : null}
            </>
          )}
        </fieldset>

        <fieldset>
          <legend>2. Choose where to save</legend>
          <p>
            Your files are plain markdown. The default folder sits outside
            Documents so iCloud will not upload them. You can move or delete
            them at any time.
          </p>
          {folder ? (
            <p className="status-line done">
              <span className="led led-on" aria-hidden="true" /> Saving to{" "}
              {folder}
            </p>
          ) : null}
          {folder && looksSynced(folder) ? (
            <p className="warn">
              This folder is inside iCloud Drive, so your files will be
              uploaded to Apple's servers. Choose a folder outside iCloud to
              keep them on this computer only.
            </p>
          ) : null}
          <div className="button-row">
            {folder ? null : (
              <button
                type="button"
                disabled={picking}
                onClick={() => void pickFolder("use_default_folder")}
              >
                Save to ~/Ambient Context
              </button>
            )}
            <button
              type="button"
              disabled={picking}
              onClick={() => void pickFolder("choose_folder")}
            >
              {picking
                ? "Choosing…"
                : folder
                  ? "Change folder"
                  : "Choose a different folder…"}
            </button>
          </div>
        </fieldset>

        {ready ? (
          <fieldset>
            <legend>You are set up</legend>
            <p>
              Click the menu bar icon to start and stop capturing, or
              right-click it and choose Start Capturing. The right-click menu
              also opens today's file and these settings.
            </p>
          </fieldset>
        ) : null}

        <p className="credit">
          Built by{" "}
          <button
            type="button"
            className="credit-link"
            onClick={() =>
              void invoke("open_link", { url: "https://twitter.com/cameronsmith" })
            }
          >
            Cameron Smith
          </button>
        </p>
      </div>

      <div className="statusbar">
        <span className="statusbar-cell">
          {capture.running ? "REC: ON" : "REC: OFF"}
        </span>
        <span className="statusbar-cell">
          {permission === "granted" ? "AX: GRANTED" : "AX: NOT GRANTED"}
        </span>
        <span className="statusbar-cell statusbar-cell-wide">
          {folder ?? "NO FOLDER SET"}
        </span>
      </div>
    </main>
  );
}
