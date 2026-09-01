import { useCallback, useEffect, useId, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "../lib/days";

/// Preferences about the application itself rather than about what it
/// records or how it summarises. One toggle today; it is where a second
/// would go, and it is why launch at login is not in the Recording group,
/// whose own note says it changes what is recorded.
export function AppSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const launchId = useId();

  const readSettings = useCallback(async () => {
    setSettings(await invoke<Settings>("get_settings"));
  }, []);

  useEffect(() => {
    void readSettings();
  }, [readSettings]);

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
    </fieldset>
  );
}
