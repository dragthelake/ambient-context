import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/* iCloud syncs anything under these paths to Apple's servers, which is the
   one place a local-only record must not quietly end up. */
export function looksSynced(folder: string): boolean {
  return (
    folder.includes("Mobile Documents") ||
    folder.includes("com~apple~CloudDocs") ||
    folder.includes("iCloud Drive")
  );
}

export function StorageSettings() {
  const [folder, setFolder] = useState<string | null>(null);
  const [picking, setPicking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void invoke<string | null>("current_folder").then(setFolder);
  }, []);

  const pick = async (command: "choose_folder" | "use_default_folder") => {
    setPicking(true);
    setError(null);
    try {
      const next = await invoke<string | null>(command);
      // choose_folder answers null when the dialog is cancelled, which is
      // not an error; the default needs no dialog, so its null is one.
      if (next) setFolder(next);
      else if (command === "use_default_folder")
        setError("Could not create the default folder.");
    } catch (failure) {
      setError(String(failure));
    } finally {
      setPicking(false);
    }
  };

  return (
    <fieldset>
      <legend>Storage</legend>
      <p className="settings-note">
        One plain markdown file per day is saved here. Changing the folder
        does not move files already written.
      </p>
      {folder ? (
        <p className="status-line done">
          <span className="led led-on" aria-hidden="true" /> Saving to{" "}
          <span className="storage-path">{folder}</span>
        </p>
      ) : (
        <p className="warn">No folder set. Nothing is being saved.</p>
      )}
      {error ? <p className="warn">{error}</p> : null}
      {folder && looksSynced(folder) ? (
        <p className="warn">
          This folder is inside iCloud Drive, so your files will be uploaded
          to Apple's servers. Choose a folder outside iCloud to keep them on
          this computer only.
        </p>
      ) : null}
      <div className="button-row">
        <button type="button" disabled={picking} onClick={() => void pick("choose_folder")}>
          {picking ? "Choosing…" : "Change folder…"}
        </button>
        <button
          type="button"
          disabled={picking}
          onClick={() => void pick("use_default_folder")}
        >
          Use Documents/Ambient Context
        </button>
      </div>
    </fieldset>
  );
}
