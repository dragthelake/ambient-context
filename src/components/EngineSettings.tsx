import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AuthState, Engine, Settings } from "../lib/days";

type Detected = {
  engine: Engine;
  auth: AuthState | null;
  test: { status: "untested" | "testing" | "ok" | "failed"; text?: string };
};

export function EngineSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [detected, setDetected] = useState<Detected[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [manualCommand, setManualCommand] = useState("");
  const [manualArgs, setManualArgs] = useState("");
  const [saving, setSaving] = useState(false);

  const readSettings = useCallback(async () => {
    const loaded = await invoke<Settings>("get_settings");
    setSettings(loaded);
    return loaded;
  }, []);

  useEffect(() => {
    void (async () => {
      const loaded = await readSettings();
      const found = await invoke<Engine[]>("engine_detect");
      setDetected(
        found.map((engine) => ({
          engine,
          auth:
            loaded.engine?.command === engine.command
              ? { state: "ok" as const }
              : null,
          test: { status: "untested" as const },
        })),
      );
      if (loaded.engine && !found.some((e) => e.command === loaded.engine?.command)) {
        setManualCommand(loaded.engine.command);
        setManualArgs(loaded.engine.args.join(" "));
      }
      if (loaded.engine) {
        setSelected(
          found.some((e) => e.command === loaded.engine?.command)
            ? loaded.engine.command
            : "manual",
        );
      }
    })();
  }, [readSettings]);

  const save = useCallback(
    async (change: (next: Settings) => Settings) => {
      if (!settings) return;
      setSaving(true);
      try {
        const next = change({ ...settings });
        await invoke("set_settings", { next });
        setSettings(next);
      } finally {
        setSaving(false);
      }
    },
    [settings],
  );

  // Whether the engine is signed in, asked once per page open, never on a
  // schedule. Never offers to sign in; names the command that fixes it.
  useEffect(() => {
    void (async () => {
      for (const entry of detected) {
        if (entry.auth !== null) continue;
        const auth = await invoke<AuthState>("engine_auth", {
          engineConfig: entry.engine,
        });
        setDetected((current) =>
          current.map((d) =>
            d.engine.command === entry.engine.command ? { ...d, auth } : d,
          ),
        );
      }
    })();
    // Run once per settings page open.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const chosenEngine = useCallback((): Engine | null => {
    if (selected === null) return null;
    if (selected === "manual") {
      if (!manualCommand.trim()) return null;
      return {
        label: "Custom engine",
        command: manualCommand.trim(),
        args: manualArgs.trim() ? manualArgs.trim().split(/\s+/) : [],
        timeout_secs: 600,
      };
    }
    return detected.find((d) => d.engine.command === selected)?.engine ?? null;
  }, [selected, manualCommand, manualArgs, detected]);

  const [test, setTest] = useState<{ status: "untested" | "testing" | "ok" | "failed"; text?: string }>({
    status: "untested",
  });

  const runTest = useCallback(async () => {
    const engine = chosenEngine();
    if (!engine) return;
    setTest({ status: "testing" });
    try {
      const out = await invoke<string>("engine_test", { engineConfig: engine });
      setTest({ status: "ok", text: out });
    } catch (error) {
      setTest({ status: "failed", text: String(error) });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [chosenEngine]);

  if (!settings) return null;

  return (
    <section className="settings-section">
      <fieldset>
        <legend>Daily summary</legend>
        <p>
          Once a day, at a time you choose, Ambient Context hands the day's
          file to a program already on this computer and saves what it writes
          next to your record. The app itself sends nothing anywhere and
          makes no model call: the run happens inside your own agent, under
          your own subscription. It reads only the day file it is
          summarising, and every run is written to that day's ledger.
        </p>

        <h3 className="settings-heading">Engine</h3>
        {detected.length === 0 ? (
          <p className="settings-note">
            No known agent CLI was found on this computer. You can still point
            the app at any program that reads a prompt on standard input.
          </p>
        ) : (
          <ul className="engine-list">
            {detected.map((entry) => (
              <li key={entry.engine.command}>
                <label className="engine-row">
                  <input
                    type="radio"
                    name="engine"
                    checked={selected === entry.engine.command}
                    onChange={() => setSelected(entry.engine.command)}
                  />
                  <span className="engine-label">{entry.engine.label}</span>
                  <span className="engine-path">{entry.engine.command}</span>
                  <EngineAuth state={entry.auth} />
                </label>
              </li>
            ))}
            <li>
              <label className="engine-row">
                <input
                  type="radio"
                  name="engine"
                  checked={selected === "manual"}
                  onChange={() => setSelected("manual")}
                />
                <span className="engine-label">Something else</span>
              </label>
              {selected === "manual" ? (
                <div className="manual-engine">
                  <label>
                    Command
                    <input
                      type="text"
                      value={manualCommand}
                      onChange={(event) => setManualCommand(event.target.value)}
                      placeholder="/usr/local/bin/my-engine"
                    />
                  </label>
                  <label>
                    Arguments
                    <input
                      type="text"
                      value={manualArgs}
                      onChange={(event) => setManualArgs(event.target.value)}
                      placeholder="--one-shot (the prompt goes in on standard input)"
                    />
                  </label>
                </div>
              ) : null}
            </li>
          </ul>
        )}

        <div className="button-row">
          <button
            type="button"
            disabled={!chosenEngine() || test.status === "testing"}
            onClick={() => void runTest()}
          >
            {test.status === "testing" ? "Testing…" : "Test engine"}
          </button>
          <button
            type="button"
            disabled={saving || chosenEngine() === null}
            onClick={async () => {
              const engine = chosenEngine();
              if (!engine) return;
              await save((next) => ({ ...next, engine }));
              setTest({ status: "untested" });
            }}
          >
            {settings.engine ? "Update engine" : "Connect engine"}
          </button>
          {settings.engine ? (
            <button
              type="button"
              disabled={saving}
              onClick={async () => {
                await save((next) => ({ ...next, engine: null }));
                setSelected(null);
              }}
            >
              Disconnect
            </button>
          ) : null}
        </div>
        {test.status === "ok" ? (
          <p className="status-line done">
            <span className="led led-on" aria-hidden="true" /> Test passed:{" "}
            {test.text}
          </p>
        ) : null}
        {test.status === "failed" ? (
          <p className="warn">Test failed: {test.text}</p>
        ) : null}
        {test.status === "untested" && settings.engine ? (
          <p className="settings-note">This engine has not passed a test yet.</p>
        ) : null}

        <h3 className="settings-heading">Schedule</h3>
        <label className="schedule-row">
          <input
            type="checkbox"
            checked={settings.schedule_hhmm !== null}
            disabled={!settings.engine}
            onChange={(event) =>
              void save((next) => ({
                ...next,
                schedule_hhmm: event.target.checked ? "06:00" : null,
              }))
            }
          />
          Summarise once a day at
        </label>
        <input
          type="time"
          value={settings.schedule_hhmm ?? "06:00"}
          disabled={!settings.engine || settings.schedule_hhmm === null}
          onChange={(event) =>
            void save((next) => ({
              ...next,
              schedule_hhmm: event.target.value || null,
            }))
          }
        />
        <p className="settings-note">
          Turning this on summarises the last seven captured days, one at a
          time. Older days can be summarised one at a time from the Day view.
        </p>

        <h3 className="settings-heading">Launch at login</h3>
        <label className="schedule-row">
          <input
            type="checkbox"
            checked={settings.launch_at_login}
            onChange={(event) =>
              void invoke("set_launch_at_login", {
                enabled: event.target.checked,
              })
                .then(readSettings)
                .catch(() => undefined)
            }
          />
          Ambient Context opens when you log in
        </label>
        <p className="settings-note">
          The app starts with the Mac so the daily summary runs whether or not
          you opened it.
        </p>

        <h3 className="settings-heading">Prompt</h3>
        <p className="settings-note">
          {settings.day_prompt
            ? `Using your own prompt at ${settings.day_prompt}`
            : "Using the built-in prompt."}
        </p>
        <div className="button-row">
          {settings.day_prompt ? (
            <button
              type="button"
              disabled={saving}
              onClick={async () =>
                await save((next) => ({ ...next, day_prompt: null }))
              }
            >
              Revert to built-in
            </button>
          ) : null}
        </div>
      </fieldset>
    </section>
  );
}

function EngineAuth({ state }: { state: AuthState | null }) {
  if (state === null) return null;
  switch (state.state) {
    case "ok":
      return null;
    case "not_logged_in":
      return (
        <span className="engine-auth">
          Not signed in. {state.fix}
        </span>
      );
    case "no_provider":
      return (
        <span className="engine-auth">
          No provider configured, opencode will answer with a free model.{" "}
          {state.fix}
        </span>
      );
    default:
      return null;
  }
}
