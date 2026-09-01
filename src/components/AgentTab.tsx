import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AuthState, Agent, Settings } from "../lib/days";
import { PromptSettings } from "./PromptSettings";

type Detected = {
  agent: Agent;
  auth: AuthState | null;
};

// Mirrors CLAUDE_MODELS and claude_code_args_for in src-tauri/src/agent.rs;
// the two lists move together. Haiku 4.5 takes no effort levels, so the
// flag only goes on the models that accept it.
const CLAUDE_MODELS = [
  { id: "claude-opus-5", label: "Opus 5 (most capable)" },
  { id: "claude-sonnet-5", label: "Sonnet 5 (balanced)" },
  { id: "claude-haiku-4-5", label: "Haiku 4.5 (fastest)" },
];

function claudeArgsFor(model: string): string[] {
  const args = ["-p", "--output-format", "text", "--model", model];
  if (model !== "claude-haiku-4-5") args.push("--effort", "medium");
  return args;
}

function claudeModelOf(args: string[]): string | null {
  const at = args.indexOf("--model");
  return at >= 0 ? (args[at + 1] ?? null) : null;
}

export function AgentTab() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [detected, setDetected] = useState<Detected[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [manualCommand, setManualCommand] = useState("");
  const [manualArgs, setManualArgs] = useState("");
  const [claudeModel, setClaudeModel] = useState(CLAUDE_MODELS[0].id);
  const [saving, setSaving] = useState(false);

  const readSettings = useCallback(async () => {
    const loaded = await invoke<Settings>("get_settings");
    setSettings(loaded);
    return loaded;
  }, []);

  useEffect(() => {
    void (async () => {
      const loaded = await readSettings();
      const found = await invoke<Agent[]>("agent_detect");
      // Every row starts unprobed, including the saved one: an agent that
      // has since been logged out must not be shown as fine.
      setDetected(found.map((agent) => ({ agent, auth: null })));
      if (loaded.agent && !found.some((a) => a.command === loaded.agent?.command)) {
        setManualCommand(loaded.agent.command);
        setManualArgs(loaded.agent.args.join(" "));
      }
      if (loaded.agent?.label === "Claude Code") {
        const model = claudeModelOf(loaded.agent.args);
        if (model && CLAUDE_MODELS.some((m) => m.id === model)) setClaudeModel(model);
      }
      if (loaded.agent) {
        setSelected(
          found.some((a) => a.command === loaded.agent?.command)
            ? loaded.agent.command
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

  const detectedCommands = detected.map((d) => d.agent.command).join("\n");

  // Whether each agent is signed in, asked once per detection, never on a
  // schedule. Never offers to sign in; names the command that fixes it.
  // Keyed on the detected commands so it runs after detection has landed.
  useEffect(() => {
    if (detectedCommands === "") return;
    let cancelled = false;
    void (async () => {
      for (const command of detectedCommands.split("\n")) {
        const entry = detected.find((d) => d.agent.command === command);
        if (!entry) continue;
        const auth = await invoke<AuthState>("agent_auth", {
          agentConfig: entry.agent,
        });
        if (cancelled) return;
        setDetected((current) =>
          current.map((d) => (d.agent.command === command ? { ...d, auth } : d)),
        );
      }
    })();
    return () => {
      cancelled = true;
    };
    // Reruns only when the set of detected agents changes, not when a probe
    // writes its answer back.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [detectedCommands]);

  const chosenAgent = useCallback((): Agent | null => {
    if (selected === null) return null;
    if (selected === "manual") {
      if (!manualCommand.trim()) return null;
      return {
        label: "Custom agent",
        command: manualCommand.trim(),
        args: manualArgs.trim() ? manualArgs.trim().split(/\s+/) : [],
        timeout_secs: 600,
      };
    }
    const found = detected.find((d) => d.agent.command === selected)?.agent ?? null;
    // The detected preset pins the default model; the picker's choice
    // replaces it for Claude Code.
    if (found?.label === "Claude Code") {
      return { ...found, args: claudeArgsFor(claudeModel) };
    }
    return found;
  }, [selected, manualCommand, manualArgs, detected, claudeModel]);

  const [test, setTest] = useState<{ status: "untested" | "testing" | "ok" | "failed"; text?: string }>({
    status: "untested",
  });

  // Which test run is current. A run that is no longer current must not
  // write its result: the user has switched provider or started another.
  const testRun = useRef(0);

  const runTest = useCallback(async () => {
    const agent = chosenAgent();
    if (!agent) return;
    const run = ++testRun.current;
    setTest({ status: "testing" });
    let result: { status: "ok" | "failed"; text: string };
    try {
      // The backend caps a test at 60 seconds and errors on its own; the
      // frontend cap behind it means "Testing…" can never be the terminal
      // state even if that reply is lost.
      const out = await Promise.race([
        invoke<string>("agent_test", { agentConfig: agent }),
        new Promise<never>((_, reject) =>
          setTimeout(() => reject("No reply after 90 seconds."), 90_000),
        ),
      ]);
      result = { status: "ok", text: out };
    } catch (error) {
      result = { status: "failed", text: String(error) };
    }
    if (testRun.current === run) setTest(result);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [chosenAgent]);

  if (!settings) return null;

  const connected = settings.agent !== null;

  const detail = (isConnectedRow: boolean) => (
    <div className="provider-detail">
      <div className="button-row">
        <button
          type="button"
          disabled={!chosenAgent() || test.status === "testing"}
          onClick={() => void runTest()}
        >
          {test.status === "testing" ? "Testing…" : "Test"}
        </button>
        {isConnectedRow ? (
          <button
            type="button"
            disabled={saving}
            onClick={async () => {
              await save((next) => ({ ...next, agent: null }));
              testRun.current += 1;
              setTest({ status: "untested" });
            }}
          >
            Disconnect
          </button>
        ) : (
          <button
            type="button"
            disabled={saving || chosenAgent() === null}
            onClick={async () => {
              const agent = chosenAgent();
              if (!agent) return;
              // The test result is kept: Connect applies the provider that
              // was just tested, so a pass is still true of it.
              await save((next) => ({ ...next, agent }));
            }}
          >
            Connect
          </button>
        )}
      </div>
      {test.status === "testing" ? (
        <p className="status-line waiting">
          Waiting for the agent to reply. This can take a minute
          <span className="blink" aria-hidden="true">
            _
          </span>
        </p>
      ) : null}
      {test.status === "ok" ? (
        <p className="status-line done">
          <span className="led led-on" aria-hidden="true" /> Test passed:{" "}
          {test.text}
        </p>
      ) : null}
      {test.status === "failed" ? (
        <p className="warn">Test failed: {test.text}</p>
      ) : null}
      {test.status === "untested" && isConnectedRow ? (
        <p className="settings-note">This provider has not passed a test yet.</p>
      ) : null}

      <h4 className="settings-heading">Schedule</h4>
      <label className="schedule-row">
        <input
          type="checkbox"
          checked={settings.schedule_hhmm !== null}
          disabled={!connected}
          onChange={(event) =>
            void save((next) => ({
              ...next,
              schedule_hhmm: event.target.checked ? "06:00" : null,
            }))
          }
        />
        Summarise once a day at
        <input
          type="time"
          value={settings.schedule_hhmm ?? "06:00"}
          disabled={!connected || settings.schedule_hhmm === null}
          onChange={(event) =>
            void save((next) => ({
              ...next,
              schedule_hhmm: event.target.value || null,
            }))
          }
        />
      </label>
      <p className="settings-note">
        {connected
          ? "Turning this on summarises up to seven recent captured days, one at a time. Older days can be summarised from the Day view."
          : "Connect this provider to turn on the daily schedule."}
      </p>
    </div>
  );

  // A row can only be chosen while nothing is connected: switching providers
  // means disconnecting first, so the saved agent is never silently replaced.
  const providerRow = (
    key: string,
    label: ReactNode,
    body: ReactNode,
  ) => {
    const isConnectedRow =
      connected &&
      (key === "manual"
        ? selected === "manual"
        : settings.agent?.command === key);
    const open = selected === key;
    const locked = connected && !isConnectedRow;
    return (
      <li
        key={key}
        className={[
          "provider",
          open ? "is-open" : "",
          locked ? "is-locked" : "",
        ]
          .filter(Boolean)
          .join(" ")}
      >
        <label className="agent-row">
          <input
            type="radio"
            name="agent"
            checked={open}
            disabled={locked}
            onChange={() => {
              setSelected(key);
              testRun.current += 1;
              setTest({ status: "untested" });
            }}
          />
          <span className="agent-label">{label}</span>
          {isConnectedRow ? (
            <span className="provider-state">
              <span className="led led-on" aria-hidden="true" /> Connected
            </span>
          ) : null}
        </label>
        {open ? (
          <>
            {body}
            {detail(isConnectedRow)}
          </>
        ) : null}
      </li>
    );
  };

  return (
    <>
      <fieldset>
        <legend>Agent</legend>
        <p>
          Once a day, your own agent reads that day's file and saves a summary
          beside it.
        </p>

        <h3 className="settings-heading">Provider</h3>
        {detected.length === 0 ? (
          <p className="settings-note">
            No known agent CLI was found on this computer. You can still point
            the app at any program that reads a prompt on standard input.
          </p>
        ) : connected ? (
          <p className="settings-note">
            To change provider, disconnect the current one first.
          </p>
        ) : null}
        <ul className="provider-list">
          {detected.map((entry) =>
            providerRow(
              entry.agent.command,
              entry.agent.label,
              <div className="provider-detail">
                <span className="agent-path">{entry.agent.command}</span>
                <AgentAuth state={entry.auth} />
                {entry.agent.label === "Claude Code" ? (
                  <label className="provider-field">
                    Model
                    <select
                      value={claudeModel}
                      disabled={connected}
                      onChange={(event) => {
                        setClaudeModel(event.target.value);
                        // A pass describes the model that answered; a
                        // different model has not passed anything.
                        setTest({ status: "untested" });
                      }}
                    >
                      {CLAUDE_MODELS.map((model) => (
                        <option key={model.id} value={model.id}>
                          {model.label}
                        </option>
                      ))}
                    </select>
                  </label>
                ) : (
                  <p className="settings-note">
                    Uses the model configured in the CLI's own settings.
                  </p>
                )}
              </div>,
            ),
          )}
          {providerRow(
            "manual",
            "Something else",
            <div className="provider-detail">
              <label className="provider-field">
                Command
                <input
                  type="text"
                  value={manualCommand}
                  disabled={connected}
                  onChange={(event) => {
                    setManualCommand(event.target.value);
                    // A pass describes the command as it was when tested;
                    // an edit makes that claim stale.
                    setTest({ status: "untested" });
                  }}
                  placeholder="/usr/local/bin/my-agent"
                />
              </label>
              <label className="provider-field">
                Arguments
                <input
                  type="text"
                  value={manualArgs}
                  disabled={connected}
                  onChange={(event) => {
                    setManualArgs(event.target.value);
                    setTest({ status: "untested" });
                  }}
                  placeholder="--one-shot (the prompt goes in on standard input)"
                />
              </label>
            </div>,
          )}
        </ul>
      </fieldset>
      <PromptSettings />
    </>
  );
}

function AgentAuth({ state }: { state: AuthState | null }) {
  if (state === null) return null;
  switch (state.state) {
    case "ok":
      return null;
    case "not_logged_in":
      return (
        <span className="agent-auth">
          Not signed in. {state.fix}
        </span>
      );
    case "no_provider":
      return (
        <span className="agent-auth">
          No provider configured, opencode will answer with a free model.{" "}
          {state.fix}
        </span>
      );
    default:
      return null;
  }
}
