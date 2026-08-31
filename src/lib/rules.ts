export type RuleTarget =
  | { app: string }
  | { website: string }
  | { title: string };

export type RuleAction = "exclude" | "headings_only" | "full";

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
};

export type RawBlock = {
  start: string;
  end: string;
  app: string;
  title: string | null;
  file: string | null;
  url: string | null;
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
