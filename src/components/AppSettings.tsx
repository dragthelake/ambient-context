import { useCallback, useEffect, useId, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "../lib/days";

/// Preferences about the application itself rather than about what it
/// records or how it summarises. Launch at login is not in the Recording
/// group, whose own note says it changes what is recorded, and neither is
/// the editor: both are about the app, not about the record.
export function AppSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [editor, setEditor] = useState("");
  const [autostartError, setAutostartError] = useState<string | null>(null);
  const launchId = useId();
  const editorId = useId();

  const readSettings = useCallback(async () => {
    const loaded = await invoke<Settings>("get_settings");
    setSettings(loaded);
    setEditor(loaded.editor ?? "");
    setAutostartError(await invoke<string | null>("autostart_error"));
  }, []);

  useEffect(() => {
    void readSettings();
  }, [readSettings]);

  /// An empty field means the system handler, which is None on the Rust
  /// side, not an empty path that would fail to open anything.
  const saveEditor = useCallback(
    async (path: string) => {
      if (!settings) return;
      const next = { ...settings, editor: path.trim() === "" ? null : path.trim() };
      await invoke("set_settings", { next });
      setSettings(next);
      setEditor(next.editor ?? "");
    },
    [settings],
  );

  if (!settings) return null;

  return (
    <fieldset>
      <legend>Application</legend>
      <div className="field-row">
        <input
          type="checkbox"
          id={launchId}
          checked={settings.launch_at_login}
          onChange={(event) =>
            void invoke("set_launch_at_login", { enabled: event.target.checked })
              .then(readSettings)
              .catch(() => undefined)
          }
        />
        <label htmlFor={launchId}>Ambient Context opens when you log in</label>
      </div>
      <p className="settings-note">
        The app starts with the Mac so the daily summary runs whether or not
        you opened it.
      </p>
      {autostartError ? (
        <p className="warn" role="alert">
          Login item could not be updated: {autostartError}
        </p>
      ) : null}

      <div className="field-row-stacked">
        <label htmlFor={editorId}>Open files with</label>
        <div className="field-row">
          <input
            id={editorId}
            type="text"
            value={editor}
            onChange={(event) => setEditor(event.target.value)}
            onBlur={() => void saveEditor(editor)}
            placeholder="/Applications/iA Writer.app"
          />
          <button
            type="button"
            onClick={() =>
              void invoke<string | null>("choose_editor").then((path) => {
                if (path) void saveEditor(path);
              })
            }
          >
            Choose…
          </button>
        </div>
        <p className="settings-note">
          The application used by every Open in editor button. Leave empty for
          the system default for markdown.
        </p>
      </div>
    </fieldset>
  );
}
