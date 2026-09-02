import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

export type PromptPayload = {
  id: string;
  text: string;
  customised: boolean;
  path: string;
};

export type PromptId =
  | "day-context"
  | "ingest-messages"
  | "ingest-apps"
  | "ingest-websites";

export function PromptSettings() {
  const [id, setId] = useState<PromptId>("day-context");
  const [payload, setPayload] = useState<PromptPayload | null>(null);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [copied, setCopied] = useState(false);

  const read = useCallback(async () => {
    const next = await invoke<PromptPayload>("get_prompt", { id });
    setPayload(next);
    setDraft(next.text);
    setConfirming(false);
  }, [id]);

  useEffect(() => {
    void read();
  }, [read]);

  const save = async () => {
    try {
      const next = await invoke<PromptPayload>("set_prompt", { id, text: draft });
      setPayload(next);
      setDraft(next.text);
      setError(null);
      setSaved(true);
      setTimeout(() => setSaved(false), 2500);
    } catch (e) {
      // The validation messages name the heading that is missing, so they
      // are shown exactly as written.
      setError(String(e));
    }
  };

  const reset = async () => {
    try {
      const next = await invoke<PromptPayload>("reset_prompt", { id });
      setPayload(next);
      setDraft(next.text);
      setError(null);
      setConfirming(false);
    } catch (e) {
      setError(String(e));
    }
  };

  const copyPath = async () => {
    if (!payload) return;
    await writeText(payload.path);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // Opens the customised prompt for editing, or a read-only copy of the
  // bundled one when nothing has been customised yet.
  const openInEditor = async () => {
    try {
      await invoke("open_prompt_in_editor", { id });
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  if (!payload) return null;

  return (
    <fieldset>
      <legend>Prompts</legend>
      <p className="settings-note">
        {payload.customised
          ? `Using your prompt at ${payload.path}`
          : "Using the built-in prompt."}
      </p>
      <p className="settings-note">
        Saving writes your own copy to that file. The agent reads it on the
        next run.
      </p>

      {error ? <p className="warn">{error}</p> : null}

      <div className="field-row-stacked">
        <label htmlFor="prompt-id">Prompt</label>
        <select
          id="prompt-id"
          value={id}
          onChange={(event) => setId(event.target.value as PromptId)}
        >
          <option value="day-context">Daily notes</option>
          <option value="ingest-messages">Ingest messages</option>
          <option value="ingest-apps">Ingest apps</option>
          <option value="ingest-websites">Ingest websites</option>
        </select>
      </div>

      <div className="field-row-stacked">
        <label htmlFor="prompt-text">Prompt text</label>
        <textarea
          id="prompt-text"
          rows={16}
          spellCheck={false}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
        />
      </div>

      <div className="button-row">
        <button type="button" onClick={() => void save()} disabled={draft === payload.text}>
          {saved ? "Saved" : "Save"}
        </button>
        <button type="button" onClick={() => void openInEditor()}>
          Open in editor
        </button>
        <button type="button" onClick={() => void copyPath()}>
          {copied ? "Path copied" : "Copy path"}
        </button>
        {payload.customised && !confirming ? (
          <button type="button" onClick={() => setConfirming(true)}>
            Reset to bundled
          </button>
        ) : null}
      </div>

      {confirming ? (
        <div className="button-row">
          <p className="settings-note">
            This replaces your prompt with the bundled one. Your version is not
            kept.
          </p>
          <button type="button" onClick={() => void reset()}>
            Reset
          </button>
          <button type="button" onClick={() => setConfirming(false)}>
            Cancel
          </button>
        </div>
      ) : null}
    </fieldset>
  );
}
