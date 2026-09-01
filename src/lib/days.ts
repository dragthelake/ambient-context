export type Engine = {
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
  engine: Engine | null;
  schedule_hhmm: string | null;
  editor: string | null;
  launch_at_login: boolean;
  max_block_chars: number;
  sound_enabled: boolean;
  sound_volume: number;
  write_references: boolean;
  extra_redaction_patterns: string[];
};

/// One day as `list_days` reports it. Mirrors `days::DayEntry` in Rust.
/// `date` arrives as the `YYYY-MM-DD` string chrono serialises a NaiveDate
/// to, not as a Date.
export type DayEntry = {
  date: string;
  has_capture: boolean;
  has_summary: boolean;
  bytes: number;
  title: string | null;
};
