export type Agent = {
  label: string;
  command: string;
  args: string[];
  timeout_secs: number;
};

export type AuthState =
  | { state: "ok" }
  | { state: "not_logged_in"; fix: string }
  | { state: "no_provider"; fix: string }
  | { state: "unknown" };

export type Settings = {
  folder: string | null;
  enabled: boolean;
  interval_secs: number;
  min_dwell_secs: number;
  similarity_threshold: number;
  agent: Agent | null;
  ingest_agent: Agent | null;
  ingest_max_chars: number;
  schedule_hhmm: string | null;
  editor: string | null;
  launch_at_login: boolean;
  idle_secs: number;
  max_block_chars: number;
  sound_enabled: boolean;
  sound_volume: number;
  write_references: boolean;
  extra_redaction_patterns: string[];
};

export type DayFile = "apps" | "websites" | "messages";

export type UrlTotal = {
  url: string;
  domain: string;
  title: string;
  dwell_secs: number;
  visits: number;
  first: string;
  last: string;
};

/// One day as `list_days` reports it. Mirrors `days::DayEntry` in Rust.
/// `date` arrives as the `YYYY-MM-DD` string chrono serialises a NaiveDate
/// to, not as a Date.
export type DayEntry = {
  date: string;
  has_capture: boolean;
  has_summary: boolean;
  has_kb: boolean;
  bytes: number;
  title: string | null;
};
