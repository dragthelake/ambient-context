export type RuleTarget =
  | { app: string }
  | { website: string }
  | { title: string };

export type RuleAction = "exclude" | "headings_only" | "full" | "route_messages";

export type Rule = {
  id: string;
  target: RuleTarget;
  action: RuleAction;
  note?: string | null;
};

export type BuiltIn = { id: string; description: string };

export type RulesPayload = {
  rules: Rule[];
  built_ins: BuiltIn[];
  next_id: string;
  /// Non-null when rules.json cannot be read. The list is empty and every
  /// write refuses with this message.
  error: string | null;
  path?: string | null;
};

export type RawBlock = {
  start: string;
  end: string;
  app: string;
  title: string | null;
  file: string | null;
  url: string | null;
  routed?: string | null;
  lines: string[];
};

/// The host of a captured url, matching rules::domain_of in Rust: a rule
/// made from a block must match the same blocks the block came from.
export function domainOf(url: string): string | null {
  try {
    const host = new URL(url).hostname.toLowerCase();
    return host.startsWith("www.") ? host.slice(4) : host;
  } catch {
    return null;
  }
}

/// Turns highlighted text into a literal regex, so "redact text like this"
/// means exactly this text and not an accidental pattern.
export function literalPattern(text: string): string {
  return text.trim().replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export type ProposeTarget = "rules" | "prompt";

export type Selection = {
  date: string;
  text: string;
  app: string | null;
  title: string | null;
  time_range: string | null;
  mode: "raw" | "summary";
};

export type Proposal = {
  id: string;
  target: ProposeTarget;
  before: string;
  after: string;
  diff: string;
  reasoning: string;
  ledger_path: string;
};

export type ProposeError =
  | { kind: "no_agent" }
  | { kind: "agent_failed"; stderr: string }
  | { kind: "invalid"; reason: string; raw: string };
