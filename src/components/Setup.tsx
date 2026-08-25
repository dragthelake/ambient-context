import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Permission = "granted" | "notGranted";

type Census = {
  app: string;
  window_title: string | null;
  element_count: number;
  character_count: number;
  walk_ms: number;
  sample: string;
};

function looksSynced(folder: string): boolean {
  return (
    folder.includes("Mobile Documents") ||
    folder.includes("com~apple~CloudDocs") ||
    folder.includes("iCloud Drive")
  );
}

export function Setup() {
  const [permission, setPermission] = useState<Permission>("notGranted");
  const [folder, setFolder] = useState<string | null>(null);
  const [census, setCensus] = useState<Census | null>(null);

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

  const ready = permission === "granted" && folder !== null;

  return (
    <main className="setup">
      <p className="eyebrow">A written record of your day</p>
      <h1>Ambient Context</h1>

      <section>
        <h2>What it does</h2>
        <p>
          While it is switched on, Ambient Context reads the text of whichever
          window you have focused and writes it to a markdown file on your
          computer. One file per day, in a folder you choose.
        </p>
        <ul>
          <li>It never takes screenshots or records your screen.</li>
          <li>It only reads the window you are actually looking at.</li>
          <li>Nothing is sent anywhere. There is no account and no server.</li>
          <li>
            It skips password fields, password managers and private browsing
            windows.
          </li>
          <li>It is off until you switch it on, every time you open it.</li>
        </ul>
      </section>

      <section>
        <h2>1. Allow access</h2>
        <p>
          macOS needs your permission before any app can read another app's
          windows. Choosing Allow opens System Settings, where you switch
          Ambient Context on.
        </p>
        {permission === "granted" ? (
          <p className="done">Access allowed.</p>
        ) : (
          <button type="button" onClick={() => invoke("request_permission")}>
            Allow access
          </button>
        )}
      </section>

      <section>
        <h2>2. Choose where to save</h2>
        <p>
          Your files are plain markdown. The default folder sits outside
          Documents so iCloud will not upload them. You can move or delete them
          at any time.
        </p>
        {folder ? <p className="done">Saving to {folder}</p> : null}
        {folder && looksSynced(folder) ? (
          <p className="warn">
            This folder is inside iCloud Drive, so your files will be uploaded
            to Apple's servers. Choose a folder outside iCloud to keep them on
            this computer only.
          </p>
        ) : null}
        {folder ? null : (
          <button
            type="button"
            onClick={async () =>
              setFolder((await invoke<string | null>("use_default_folder")) ?? null)
            }
          >
            Save to ~/Ambient Context
          </button>
        )}
        <button
          type="button"
          className={folder ? undefined : "secondary"}
          onClick={async () =>
            setFolder((await invoke<string | null>("choose_folder")) ?? folder)
          }
        >
          {folder ? "Change folder" : "Choose a different folder…"}
        </button>
      </section>

      {ready ? (
        <section>
          <h2>You are set up</h2>
          <p>
            Click the menu bar icon to start and stop capturing. Right-click it
            to open today's file or change these settings.
          </p>
        </section>
      ) : null}

      <section className="census">
        <h2>Coverage check</h2>
        <p>
          Focus an app, then sample it. Enable Chromium first if the tree looks
          empty.
        </p>
        <div className="row">
          <button
            type="button"
            className="secondary"
            onClick={async () => setCensus(await invoke<Census | null>("census_snapshot"))}
          >
            Sample focused window
          </button>
          <button
            type="button"
            className="secondary"
            onClick={() => invoke("enable_frontmost_accessibility")}
          >
            Enable Chromium access
          </button>
        </div>
        {census ? (
          <pre>
            {census.app}
            {census.window_title ? ` · ${census.window_title}` : ""}
            {"\n"}
            {census.element_count} fragments · {census.character_count} chars ·{" "}
            {census.walk_ms} ms
            {"\n"}
            {census.sample}
          </pre>
        ) : null}
      </section>
    </main>
  );
}
