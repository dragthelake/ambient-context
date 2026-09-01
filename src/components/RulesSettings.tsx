import { useCallback, useEffect, useId, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { RuleAction, RuleTarget, RulesPayload } from "../lib/rules";

const ACTION_LABELS: Record<RuleAction, string> = {
  exclude: "Don't record",
  headings_only: "Headings only",
  full: "Record fully",
};

function targetParts(target: RuleTarget): { kind: string; value: string } {
  if ("app" in target) return { kind: "Application", value: target.app };
  if ("website" in target) return { kind: "Website", value: target.website };
  return { kind: "Window title", value: target.title };
}

function builtInTitle(id: string): string {
  switch (id) {
    case "builtin:password-managers":
      return "Password managers";
    case "builtin:private-windows":
      return "Private browsing";
    case "builtin:secure-fields":
      return "Secure text fields";
    case "builtin:secret-patterns":
      return "Secrets and card numbers";
    default:
      return id.replace("builtin:", "");
  }
}

export function RulesSettings() {
  const [payload, setPayload] = useState<RulesPayload | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [kind, setKind] = useState<"app" | "website" | "title">("app");
  const [pattern, setPattern] = useState("");
  const [action, setAction] = useState<RuleAction>("exclude");
  const [note, setNote] = useState("");
  const kindId = useId();
  const patternId = useId();
  const actionId = useId();
  const noteId = useId();

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
      <p className="settings-note">
        Rules match an application, website domain or window title. They apply
        on top of the built-in protections below.
      </p>

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
        <p className="settings-note rules-empty">
          No rules yet. Everything not built in is recorded in full.
        </p>
      ) : (
        <ul className="rules-list">
          {payload.rules.map((rule) => (
            <RuleRow
              key={rule.id}
              rule={rule}
              onUpdate={update}
              onRemove={remove}
            />
          ))}
        </ul>
      )}

      {adding && payload.error === null ? (
        <div className="field-border rule-add">
          <h3 className="settings-heading">New rule</h3>
          <div className="field-row-stacked">
            <label htmlFor={kindId}>Matches</label>
            <select id={kindId} value={kind} onChange={(event) => setKind(event.target.value as typeof kind)}>
              <option value="app">Application</option>
              <option value="website">Website domain</option>
              <option value="title">Window title</option>
            </select>
          </div>
          <div className="field-row-stacked">
            <label htmlFor={patternId}>
              {kind === "app" ? "Application name" : kind === "website" ? "Domain" : "Title text"}
            </label>
            <input
              id={patternId}
              type="text"
              value={pattern}
              onChange={(event) => setPattern(event.target.value)}
              placeholder={kind === "website" ? "example.com" : "Slack"}
            />
          </div>
          <div className="field-row-stacked">
            <label htmlFor={actionId}>Action</label>
            <select id={actionId} value={action} onChange={(event) => setAction(event.target.value as RuleAction)}>
              {(Object.keys(ACTION_LABELS) as RuleAction[]).map((key) => (
                <option key={key} value={key}>
                  {ACTION_LABELS[key]}
                </option>
              ))}
            </select>
          </div>
          <div className="field-row-stacked">
            <label htmlFor={noteId}>Note (optional)</label>
            <input id={noteId} type="text" value={note} onChange={(event) => setNote(event.target.value)} />
          </div>
          <div className="button-row">
            <button type="button" onClick={() => void add()} disabled={pattern.trim() === ""}>
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
          <li key={builtIn.id} className="field-border-disabled rule-card is-locked">
            <div className="rule-card-head">
              <span className="rule-kind">{builtInTitle(builtIn.id)}</span>
            </div>
            <p className="rule-note">{builtIn.description}</p>
          </li>
        ))}
      </ul>
    </fieldset>
  );
}

function RuleRow({
  rule,
  onUpdate,
  onRemove,
}: {
  rule: RulesPayload["rules"][number];
  onUpdate: (id: string, action: RuleAction) => void;
  onRemove: (id: string) => void;
}) {
  const actionId = useId();
  const { kind: targetKind, value } = targetParts(rule.target);

  return (
    <li className="field-border rule-card">
      <div className="rule-card-head">
        <span className="rule-kind">{targetKind}</span>
        <span className="rule-value">{value}</span>
      </div>
      <div className="rule-card-body">
        <div className="field-row">
          <label htmlFor={actionId}>Action</label>
          <select
            id={actionId}
            value={rule.action}
            onChange={(event) => void onUpdate(rule.id, event.target.value as RuleAction)}
          >
            {(Object.keys(ACTION_LABELS) as RuleAction[]).map((key) => (
              <option key={key} value={key}>
                {ACTION_LABELS[key]}
              </option>
            ))}
          </select>
        </div>
        {rule.note ? <span className="rule-note">{rule.note}</span> : null}
        <button type="button" className="rule-remove" onClick={() => void onRemove(rule.id)}>
          Remove
        </button>
      </div>
    </li>
  );
}
