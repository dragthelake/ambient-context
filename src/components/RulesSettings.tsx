import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { RuleAction, RuleTarget, RulesPayload } from "../lib/rules";

function targetLabel(target: RuleTarget): string {
  if ("app" in target) return `app: ${target.app}`;
  if ("website" in target) return `site: ${target.website}`;
  return `title: ${target.title}`;
}

export function RulesSettings() {
  const [payload, setPayload] = useState<RulesPayload | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [kind, setKind] = useState<"app" | "website" | "title">("app");
  const [pattern, setPattern] = useState("");
  const [action, setAction] = useState<RuleAction>("exclude");
  const [note, setNote] = useState("");

  const read = useCallback(async () => {
    const next = await invoke<RulesPayload>("get_rules");
    setPayload(next);
  }, []);

  useEffect(() => {
    void read();
  }, [read]);

  const add = async () => {
    if (!payload || pattern.trim() === "") return;
    const target: RuleTarget =
      kind === "app" ? { app: pattern } : kind === "website" ? { website: pattern } : { title: pattern };
    try {
      const next = await invoke<RulesPayload>("add_rule", {
        rule: { id: payload.next_id, target, action, note: note || null },
      });
      setPayload(next);
      setError(null);
      setPattern("");
      setNote("");
      setAdding(false);
    } catch (e) {
      setError(String(e));
    }
  };

  const update = async (id: string, nextAction: RuleAction) => {
    if (!payload) return;
    const rule = payload.rules.find((r) => r.id === id);
    if (!rule) return;
    try {
      const next = await invoke<RulesPayload>("update_rule", {
        rule: { ...rule, action: nextAction },
      });
      setPayload(next);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const remove = async (id: string) => {
    try {
      const next = await invoke<RulesPayload>("remove_rule", { id });
      setPayload(next);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  if (!payload) return null;

  return (
    <fieldset>
      <legend>Capture rules</legend>
      {error ? <p className="warn">{error}</p> : null}
      {payload.error ? (
        <div className="rules-unreadable">
          <p className="warn">
            Your rules file could not be read, so none of your own rules are
            in force. The built-in protections below still apply. Fix the file
            and reopen this page.
          </p>
          <pre className="day-error">{payload.error}</pre>
          {payload.path ? <p className="settings-note">{payload.path}</p> : null}
        </div>
      ) : payload.rules.length === 0 ? (
        <p className="settings-note">
          No rules yet. Everything not built in is recorded in full.
        </p>
      ) : (
        <ul className="rules-list">
          {payload.rules.map((rule) => (
            <li key={rule.id} className="rule-row">
              <span className="rule-target">{targetLabel(rule.target)}</span>
              <select
                value={rule.action}
                onChange={(event) => void update(rule.id, event.target.value as RuleAction)}
              >
                <option value="exclude">exclude</option>
                <option value="headings_only">headings only</option>
                <option value="full">full capture</option>
              </select>
              {rule.note ? <span className="rule-note">{rule.note}</span> : null}
              <button type="button" className="rule-remove" onClick={() => void remove(rule.id)}>
                Remove
              </button>
            </li>
          ))}
        </ul>
      )}

      {adding && payload.error === null ? (
        <div className="rule-add">
          <label>
            Matches
            <select value={kind} onChange={(event) => setKind(event.target.value as typeof kind)}>
              <option value="app">application</option>
              <option value="website">website domain</option>
              <option value="title">window title</option>
            </select>
          </label>
          <label>
            {kind === "app" ? "Application name" : kind === "website" ? "Domain" : "Title text"}
            <input
              type="text"
              value={pattern}
              onChange={(event) => setPattern(event.target.value)}
              placeholder={kind === "website" ? "example.com" : "Slack"}
            />
          </label>
          <label>
            Action
            <select value={action} onChange={(event) => setAction(event.target.value as RuleAction)}>
              <option value="exclude">exclude</option>
              <option value="headings_only">headings only</option>
              <option value="full">full capture</option>
            </select>
          </label>
          <label>
            Note (optional)
            <input type="text" value={note} onChange={(event) => setNote(event.target.value)} />
          </label>
          <div className="button-row">
            <button type="button" onClick={() => void add()}>
              Add rule
            </button>
            <button type="button" onClick={() => setAdding(false)}>
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <button
          type="button"
          disabled={payload.error !== null}
          onClick={() => setAdding(true)}
        >
          Add a rule
        </button>
      )}

      <h3 className="settings-heading">Built-in protections</h3>
      <p className="settings-note">
        Always on. These cannot be changed, here or by an agent.
      </p>
      <ul className="rules-list">
        {payload.built_ins.map((builtIn) => (
          <li key={builtIn.id} className="rule-row is-locked">
            <span className="rule-target">{builtIn.id}</span>
            <span className="rule-note">{builtIn.description}</span>
          </li>
        ))}
      </ul>
    </fieldset>
  );
}
