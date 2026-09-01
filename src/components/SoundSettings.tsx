import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { applySoundSettings, play } from "../lib/sound";
import type { Settings } from "../lib/days";

export function SoundSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);

  useEffect(() => {
    void invoke<Settings>("get_settings").then(setSettings);
  }, []);

  // The engine is updated before the write is awaited, so the cue that
  // confirms a change is already playing at the new setting.
  const save = useCallback(
    async (change: (next: Settings) => Settings) => {
      if (!settings) return;
      const next = change({ ...settings });
      setSettings(next);
      applySoundSettings(next.sound_enabled, next.sound_volume);
      await invoke("set_settings", { next });
    },
    [settings],
  );

  if (!settings) return null;

  return (
    <fieldset>
      <legend>Sound</legend>
      <p className="settings-note">
        Short cues on the actions you take: starting and stopping recording,
        changing tab. Nothing is played in the background.
      </p>

      <label className="schedule-row">
        <input
          type="checkbox"
          checked={settings.sound_enabled}
          onChange={(event) => {
            const enabled = event.target.checked;
            void save((next) => ({ ...next, sound_enabled: enabled }));
            // Only on the way on: a cue confirming that sound is off would
            // be a contradiction.
            if (enabled) play("chime");
          }}
        />
        Play interface sounds
      </label>

      <label className="schedule-row volume-row">
        Volume
        <input
          type="range"
          min={0}
          max={1}
          step={0.05}
          value={settings.sound_volume}
          disabled={!settings.sound_enabled}
          aria-valuetext={`${Math.round(settings.sound_volume * 100)} percent`}
          onChange={(event) =>
            void save((next) => ({
              ...next,
              sound_volume: Number(event.target.value),
            }))
          }
          // On release rather than on every step: dragging a slider that
          // fires a cue per pixel is unpleasant.
          onPointerUp={() => settings.sound_enabled && play("tick")}
        />
        <span className="volume-readout">
          {Math.round(settings.sound_volume * 100)}%
        </span>
      </label>
    </fieldset>
  );
}
