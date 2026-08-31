import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "../lib/days";

export function RecordingSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [patterns, setPatterns] = useState("");

  const read = useCallback(async () => {
    const loaded = await invoke<Settings>("get_settings");
    setSettings(loaded);
    setPatterns(loaded.extra_redaction_patterns.join("\n"));
  }, []);

  useEffect(() => {
    void read();
  }, [read]);

  const save = useCallback(async (change: (next: Settings) => Settings) => {
    if (!settings) return;
    const next = change({ ...settings });
    await invoke("set_settings", { next });
    setSettings(next);
  }, [settings]);

  if (!settings) return null;

  return (
    <fieldset>
      <legend>Recording</legend>
      <p className="settings-note">
        These change what is recorded from now on. They never touch what is
        already written.
      </p>

      <NumberField
        label="How often the focused window is read, in seconds"
        value={settings.interval_secs}
        onChange={(value) => void save((next) => ({ ...next, interval_secs: value }))}
      />
      <NumberField
        label="How long you must stay in a window before it counts, in seconds"
        value={settings.min_dwell_secs}
        onChange={(value) => void save((next) => ({ ...next, min_dwell_secs: value }))}
      />
      <NumberField
        label="How much the text must change to start a new block, 0 to 1"
        value={settings.similarity_threshold}
        step={0.05}
        onChange={(value) => void save((next) => ({ ...next, similarity_threshold: value }))}
      />
      <NumberField
        label="The longest a single block's text can be, in characters (0 is unlimited)"
        value={settings.max_block_chars}
        onChange={(value) => void save((next) => ({ ...next, max_block_chars: value }))}
      />

      <label className="schedule-row">
        <input
          type="checkbox"
          checked={settings.write_references}
          onChange={(event) =>
            void save((next) => ({ ...next, write_references: event.target.checked }))
          }
        />
        Record the file: and url: reference for each block
      </label>

      <label className="patterns-field">
        Extra redaction patterns, one regular expression per line, replaced
        with [redacted]
        <textarea
          rows={4}
          value={patterns}
          onChange={(event) => setPatterns(event.target.value)}
          onBlur={() =>
            void save((next) => ({
              ...next,
              extra_redaction_patterns: patterns
                .split("\n")
                .map((line) => line.trim())
                .filter((line) => line !== ""),
            }))
          }
        />
      </label>
      <p className="settings-note">
        Changed patterns take effect on the next poll. No restart needed.
      </p>
    </fieldset>
  );
}

function NumberField({
  label,
  value,
  step,
  onChange,
}: {
  label: string;
  value: number;
  step?: number;
  onChange: (value: number) => void;
}) {
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);
  return (
    <label className="number-field">
      {label}
      <span className="number-current">Currently {value}</span>
      <input
        type="number"
        step={step ?? 1}
        value={text}
        onChange={(event) => setText(event.target.value)}
        onBlur={() => {
          const parsed = Number(text);
          if (!Number.isNaN(parsed)) onChange(parsed);
          else setText(String(value));
        }}
      />
    </label>
  );
}
