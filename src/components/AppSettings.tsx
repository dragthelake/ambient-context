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
  const updatesId = useId();
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
  ///
  /// Read before write, for the reason `RecordingSettings` gives: this panel
  /// sends the whole settings object, so anything another panel changed
  /// since mount would be put back from a stale snapshot.
  const saveEditor = useCallback(async (path: string) => {
    const current = await invoke<Settings>("get_settings");
    const next = { ...current, editor: path.trim() === "" ? null : path.trim() };
    await invoke("set_settings", { next });
    setSettings(next);
    setEditor(next.editor ?? "");
  }, []);

  /// Read before write, as `saveEditor` does and for the same reason.
  const saveCheckUpdates = useCallback(async (enabled: boolean) => {
    const current = await invoke<Settings>("get_settings");
    const next = { ...current, check_updates: enabled };
    await invoke("set_settings", { next });
    setSettings(next);
    setEditor(next.editor ?? "");
  }, []);

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
              // A refusal is shown, not swallowed: the setting is saved
              // before the OS answers, so the box would otherwise claim a
              // login item the machine does not have.
              .catch((error) => setAutostartError(String(error)))
              .then(readSettings)
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

      <div className="field-row">
        <input
          type="checkbox"
          id={updatesId}
          checked={settings.check_updates}
          onChange={(event) => void saveCheckUpdates(event.target.checked)}
        />
        <label htmlFor={updatesId}>Check for updates automatically</label>
      </div>
      <p className="settings-note">
        Asks GitHub Releases for a newer version shortly after launch and
        every six hours. No capture content is sent; GitHub sees the request
        itself, as with any download. You can always check from the menu
        bar.
      </p>

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
