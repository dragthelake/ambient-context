import { useCallback, useEffect, useId, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "../lib/days";

export function RecordingSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [patterns, setPatterns] = useState("");
  const writeRefsId = useId();
  const patternsId = useId();

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

      <h3 className="settings-heading">Timing</h3>
      <p className="settings-note">
        When and how often the focused window is sampled.
      </p>
      <div className="recording-fields">
        <NumberField
          label="Poll interval (seconds)"
          hint="How often the focused window is read."
          value={settings.interval_secs}
          onChange={(value) => void save((next) => ({ ...next, interval_secs: value }))}
        />
        <NumberField
          label="Minimum focus time (seconds)"
          hint="How long you must stay in a window before it counts."
          value={settings.min_dwell_secs}
          onChange={(value) => void save((next) => ({ ...next, min_dwell_secs: value }))}
        />
      </div>

      <h3 className="settings-heading">Blocks</h3>
      <p className="settings-note">
        How captured text is split and what each block stores.
      </p>
      <div className="recording-fields">
        <NumberField
          label="Change threshold (0 to 1)"
          hint="How much the text must change to start a new block."
          value={settings.similarity_threshold}
          step={0.05}
          onChange={(value) =>
            void save((next) => ({ ...next, similarity_threshold: value }))
          }
        />
        <NumberField
          label="Maximum block length (0 is unlimited)"
          hint="The longest a single block's text can be, in characters."
          value={settings.max_block_chars}
          onChange={(value) => void save((next) => ({ ...next, max_block_chars: value }))}
        />
      </div>

      <div className="field-row">
        <input
          type="checkbox"
          id={writeRefsId}
          checked={settings.write_references}
          onChange={(event) =>
            void save((next) => ({ ...next, write_references: event.target.checked }))
          }
        />
        <label htmlFor={writeRefsId}>
          Record the file: and url: reference for each block
        </label>
      </div>

      <h3 className="settings-heading">Redaction</h3>
      <div className="field-row-stacked">
        <label htmlFor={patternsId}>Extra patterns, one regular expression per line</label>
        <textarea
          id={patternsId}
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
          placeholder="Each match is replaced with [redacted]"
        />
        <p className="settings-note">
          Changed patterns take effect on the next poll. No restart needed.
        </p>
      </div>
    </fieldset>
  );
}

function NumberField({
  label,
  hint,
  value,
  step,
  onChange,
}: {
  label: string;
  hint: string;
  value: number;
  step?: number;
  onChange: (value: number) => void;
}) {
  const id = useId();
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);
  return (
    <div className="field-row-stacked">
      <label htmlFor={id}>{label}</label>
      <input
        id={id}
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
      <p className="settings-note">{hint}</p>
    </div>
  );
}
