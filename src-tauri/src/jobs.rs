use chrono::{DateTime, Local, NaiveDate, NaiveTime, TimeZone};

/// A first run after months of capture would otherwise summarise every day
/// at once, on the user's own subscription. Older days stay available to
/// summarise on demand from the Day view.
pub const MAX_BACKFILL_DAYS: usize = 7;

/// The days that should be summarised now. Empty when no schedule is set,
/// when the scheduled moment has not passed since the last run, or when
/// every finished day already has a summary.
pub fn due(
    now: DateTime<Local>,
    schedule_hhmm: Option<&str>,
    last_run: Option<DateTime<Local>>,
    captured: &[NaiveDate],
    summarised: &[NaiveDate],
) -> Vec<NaiveDate> {
    let Some(raw) = schedule_hhmm else {
        return Vec::new();
    };
    let Ok(time) = NaiveTime::parse_from_str(raw, "%H:%M") else {
        return Vec::new();
    };

    let today = now.date_naive();
    // The most recent occurrence of the scheduled time at or before now.
    let occurrence_date = if now.time() >= time {
        today
    } else {
        today.pred_opt().unwrap_or(today)
    };
    let Some(occurrence) = Local
        .from_local_datetime(&occurrence_date.and_time(time))
        .single()
    else {
        // Ambiguous or skipped local time across a daylight-saving change.
        // Treat it as not due rather than guessing; the next tick resolves.
        return Vec::new();
    };

    let overdue = match last_run {
        None => true,
        Some(last) => last < occurrence,
    };
    if !overdue {
        return Vec::new();
    }

    let mut pending: Vec<NaiveDate> = captured
        .iter()
        .copied()
        .filter(|date| *date < today && !summarised.contains(date))
        .collect();
    pending.sort();
    if pending.len() > MAX_BACKFILL_DAYS {
        pending = pending.split_off(pending.len() - MAX_BACKFILL_DAYS);
    }
    pending
}

use crate::{engine, ledger, settings, summarise, writer};
use serde::Serialize;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize)]
pub struct Outcome {
    pub when: DateTime<Local>,
    pub date: NaiveDate,
    pub ok: bool,
    pub message: String,
}

#[derive(Clone, Default)]
pub struct JobState {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    running: bool,
    last_run: Option<DateTime<Local>>,
    last_outcome: Option<Outcome>,
}

impl JobState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_running(&self) -> bool {
        self.inner.lock().map(|i| i.running).unwrap_or(false)
    }

    pub fn last_outcome(&self) -> Option<Outcome> {
        self.inner.lock().ok().and_then(|i| i.last_outcome.clone())
    }

    fn last_run(&self) -> Option<DateTime<Local>> {
        self.inner.lock().ok().and_then(|i| i.last_run)
    }

    fn set_running(&self, running: bool) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.running = running;
        }
    }

    fn record(&self, outcome: Outcome) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.last_run = Some(outcome.when);
            inner.last_outcome = Some(outcome);
        }
    }
}

/// A ledger write that fails must not lose the summary, and must not vanish
/// either. Log it and carry on.
fn record_in_ledger(folder: &Path, entry: &ledger::Entry) {
    if let Err(error) = ledger::append(folder, entry) {
        eprintln!("[ledger] could not write an entry: {error}");
    }
}

/// Summarises one day: read the day file, build the prompt, run the engine,
/// validate, write, ledger. Every path through this function writes exactly
/// one ledger entry, and every failure returns a sentence a person can act
/// on.
pub fn summarise_day(
    folder: &Path,
    engine_config: &settings::Engine,
    template: &str,
    date: NaiveDate,
    trigger: ledger::Trigger,
    reject_dir: &Path,
) -> Result<(), String> {
    let day_path = writer::file_path(folder, date);
    let day_markdown = std::fs::read_to_string(&day_path)
        .map_err(|_| format!("there is no capture for {date}"))?;

    let mut entry = ledger::Entry {
        // The moment the run started, so a reader can line an entry up
        // against the day file's own headings.
        at: Local::now(),
        trigger,
        action: "summarise_day".to_string(),
        prompt_id: Some("day-context".to_string()),
        prompt_sha256: Some(ledger::sha256_of(template.as_bytes())),
        engine: Some(engine_config.label.clone()),
        inputs: ledger::hash_file(&day_path).map(|i| vec![i]).unwrap_or_default(),
        output: None,
        reasoning: None,
        disposition: ledger::Disposition::Accepted,
    };

    let prompt = summarise::build_prompt(template, date, &day_markdown);
    let env = engine::login_shell_env();

    let output = match engine::run(engine_config, &prompt, &env) {
        Ok(output) => output,
        Err(error) => {
            let message = error.to_string();
            entry.disposition = ledger::Disposition::Failed {
                stderr: message.clone(),
            };
            record_in_ledger(folder, &entry);
            return Err(message);
        }
    };

    entry.output = Some(output.clone());
    entry.reasoning = summarise::reasoning_of(&output);

    if let Err(invalid) = summarise::validate(&output, summarise::MAX_SUMMARY_LINES) {
        // Keep the rejected output where it can be inspected, but never
        // beside the record.
        let _ = std::fs::create_dir_all(reject_dir);
        let _ = std::fs::write(reject_dir.join(format!("{date}.md")), &output);
        entry.disposition = ledger::Disposition::Rejected {
            reason: invalid.to_string(),
        };
        record_in_ledger(folder, &entry);
        return Err(format!("{invalid}; the output was kept for inspection"));
    }

    summarise::write_summary(folder, date, &output)
        .map_err(|e| format!("the summary could not be written: {e}"))?;
    entry.disposition = ledger::Disposition::Accepted;
    record_in_ledger(folder, &entry);
    Ok(())
}

pub fn run_one(app: &AppHandle, date: NaiveDate, trigger: ledger::Trigger) -> Result<(), String> {
    let config = settings::load(app);
    let folder = config.folder.clone().ok_or("no capture folder is set")?;
    let engine_config = config.engine.clone().ok_or("no engine is connected")?;
    let template = match &config.day_prompt {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|e| format!("your prompt file could not be read: {e}"))?,
        None => summarise::BUNDLED_PROMPT.to_string(),
    };
    let reject_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("rejected");
    summarise_day(&folder, &engine_config, &template, date, trigger, &reject_dir)
}

/// Ticks once a minute. A minute is fine granularity for a daily job and
/// costs nothing; it also means a machine waking at 09:14 catches up within
/// a minute rather than waiting for a precise instant it already missed.
const TICK_SECS: u64 = 60;

pub fn start(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(TICK_SECS));
        tick(&app);
    });
}

fn tick(app: &AppHandle) {
    let state = app.state::<JobState>().inner().clone();
    if state.is_running() {
        return;
    }

    let config = settings::load(app);
    let (Some(folder), Some(_)) = (config.folder.clone(), config.engine.clone()) else {
        return;
    };

    let pending = due(
        Local::now(),
        config.schedule_hhmm.as_deref(),
        state.last_run(),
        &summarise::list_captured(&folder),
        &summarise::list_summarised(&folder),
    );
    if pending.is_empty() {
        return;
    }

    state.set_running(true);
    for date in pending {
        let result = run_one(app, date, ledger::Trigger::Schedule);
        let outcome = Outcome {
            when: Local::now(),
            date,
            ok: result.is_ok(),
            message: match &result {
                Ok(()) => format!("Summarised {date}"),
                Err(message) => format!("{date} failed: {message}"),
            },
        };
        state.record(outcome);
        if result.is_err() {
            // One failure is usually the engine, and every following day
            // would fail the same way. Stop and let the user see it.
            break;
        }
    }
    state.set_running(false);
    crate::tray::refresh(app, app.state::<crate::capture::CaptureState>().is_running());
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, NaiveDate, TimeZone};

    fn at(y: i32, m: u32, d: u32, hh: u32, mm: u32) -> chrono::DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, hh, mm, 0).unwrap()
    }

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn nothing_is_due_without_a_schedule() {
        let captured = vec![day(2026, 8, 28)];
        assert!(due(at(2026, 8, 29, 9, 0), None, None, &captured, &[]).is_empty());
    }

    #[test]
    fn nothing_is_due_before_the_scheduled_time_when_it_already_ran_yesterday() {
        let captured = vec![day(2026, 8, 28)];
        let last = Some(at(2026, 8, 28, 6, 0));
        assert!(due(at(2026, 8, 29, 5, 0), Some("06:00"), last, &captured, &[]).is_empty());
    }

    #[test]
    fn yesterday_is_due_once_the_scheduled_time_has_passed() {
        let captured = vec![day(2026, 8, 28)];
        let last = Some(at(2026, 8, 28, 6, 0));
        let out = due(at(2026, 8, 29, 6, 1), Some("06:00"), last, &captured, &[]);
        assert_eq!(out, vec![day(2026, 8, 28)]);
    }

    #[test]
    fn today_is_never_summarised_because_the_day_is_not_finished() {
        let captured = vec![day(2026, 8, 28), day(2026, 8, 29)];
        let out = due(at(2026, 8, 29, 7, 0), Some("06:00"), None, &captured, &[]);
        assert_eq!(out, vec![day(2026, 8, 28)]);
    }

    #[test]
    fn days_that_already_have_summaries_are_skipped() {
        let captured = vec![day(2026, 8, 27), day(2026, 8, 28)];
        let summarised = vec![day(2026, 8, 27)];
        let out = due(
            at(2026, 8, 29, 7, 0),
            Some("06:00"),
            None,
            &captured,
            &summarised,
        );
        assert_eq!(out, vec![day(2026, 8, 28)]);
    }

    #[test]
    fn a_first_run_backfills_but_not_without_limit() {
        // Enabling summaries after months of capture must not spend months
        // of the user's tokens in one morning.
        let captured: Vec<NaiveDate> = (1..=28).map(|d| day(2026, 8, d)).collect();
        let out = due(at(2026, 8, 29, 7, 0), Some("06:00"), None, &captured, &[]);
        assert_eq!(out.len(), MAX_BACKFILL_DAYS);
        assert_eq!(*out.last().unwrap(), day(2026, 8, 28));
    }

    #[test]
    fn due_days_come_back_oldest_first() {
        let captured = vec![day(2026, 8, 26), day(2026, 8, 27), day(2026, 8, 28)];
        let out = due(at(2026, 8, 29, 7, 0), Some("06:00"), None, &captured, &[]);
        assert_eq!(
            out,
            vec![day(2026, 8, 26), day(2026, 8, 27), day(2026, 8, 28)]
        );
    }

    #[test]
    fn a_missed_night_catches_up_rather_than_being_skipped() {
        // Machine asleep at 06:00, opened at 14:00 the next day.
        let captured = vec![day(2026, 8, 27), day(2026, 8, 28)];
        let last = Some(at(2026, 8, 27, 6, 0));
        let out = due(at(2026, 8, 29, 14, 0), Some("06:00"), last, &captured, &[]);
        assert_eq!(out, vec![day(2026, 8, 27), day(2026, 8, 28)]);
    }

    #[test]
    fn a_malformed_schedule_string_means_no_schedule_rather_than_a_panic() {
        let captured = vec![day(2026, 8, 28)];
        assert!(
            due(at(2026, 8, 29, 9, 0), Some("nonsense"), None, &captured, &[]).is_empty()
        );
    }

    use tempfile::tempdir;

    fn stub_engine(command: &str, args: &[&str]) -> crate::settings::Engine {
        crate::settings::Engine {
            label: "stub".to_string(),
            command: command.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
            timeout_secs: 10,
        }
    }

    fn write_day(folder: &std::path::Path, date: NaiveDate) {
        std::fs::write(
            crate::writer::file_path(folder, date),
            "---\ndate: 2026-08-28\n---\n\n## 09:00\u{2013}11:00 \u{00b7} Linear\n\nread the issue\n",
        )
        .unwrap();
    }

    fn valid_summary() -> String {
        [
            "---",
            "date: 2026-08-28",
            "type: day-context",
            "generated_by: stub",
            "---",
            "",
            "# A day of plumbing",
            "",
            "## Sessions",
            "09:00-11:00 building the thing.",
            "",
            "## Reasoning",
            "Kept the long block.",
        ]
        .join("\n")
    }

    #[test]
    fn a_malformed_run_writes_one_ledger_entry_and_no_summary() {
        let folder = tempdir().unwrap();
        let rejects = tempdir().unwrap();
        write_day(folder.path(), day(2026, 8, 28));

        // /bin/echo ignores stdin and prints its argument: an engine that
        // answers, badly.
        let error = summarise_day(
            folder.path(),
            &stub_engine("/bin/echo", &["not a summary"]),
            "{{DATE}}\n{{DAY_FILE}}",
            day(2026, 8, 28),
            crate::ledger::Trigger::Schedule,
            rejects.path(),
        )
        .unwrap_err();

        assert!(error.contains("frontmatter"), "error was {error:?}");
        assert!(!crate::summarise::summary_path(folder.path(), day(2026, 8, 28)).exists());
        assert!(rejects.path().join("2026-08-28.md").exists());

        let ledger_file =
            crate::ledger::ledger_path(folder.path(), Local::now().date_naive());
        let text = std::fs::read_to_string(ledger_file).unwrap();
        assert_eq!(text.matches("\n## ").count(), 1, "exactly one entry");
        assert!(text.contains("rejected:"));
    }

    #[test]
    fn an_engine_that_never_returns_an_answer_is_still_ledgered() {
        let folder = tempdir().unwrap();
        let rejects = tempdir().unwrap();
        write_day(folder.path(), day(2026, 8, 28));

        let error = summarise_day(
            folder.path(),
            &stub_engine("/bin/sh", &["-c", "echo not logged in >&2; exit 1"]),
            "{{DAY_FILE}}",
            day(2026, 8, 28),
            crate::ledger::Trigger::Schedule,
            rejects.path(),
        )
        .unwrap_err();

        assert!(error.contains("not logged in"), "error was {error:?}");
        let ledger_file =
            crate::ledger::ledger_path(folder.path(), Local::now().date_naive());
        let text = std::fs::read_to_string(ledger_file).unwrap();
        assert_eq!(text.matches("\n## ").count(), 1);
        assert!(text.contains("failed:"));
    }

    #[test]
    fn a_valid_run_writes_the_summary_and_an_accepted_entry_with_its_reasoning() {
        let folder = tempdir().unwrap();
        let rejects = tempdir().unwrap();
        write_day(folder.path(), day(2026, 8, 28));

        // /bin/cat returns whatever the prompt was, so a template with no
        // placeholders is a stub engine that answers correctly.
        summarise_day(
            folder.path(),
            &stub_engine("/bin/cat", &[]),
            &valid_summary(),
            day(2026, 8, 28),
            crate::ledger::Trigger::OnDemand,
            rejects.path(),
        )
        .unwrap();

        let summary = std::fs::read_to_string(
            crate::summarise::summary_path(folder.path(), day(2026, 8, 28)),
        )
        .unwrap();
        assert!(summary.contains("# A day of plumbing"));

        let ledger_file =
            crate::ledger::ledger_path(folder.path(), Local::now().date_naive());
        let text = std::fs::read_to_string(ledger_file).unwrap();
        assert!(text.contains("- disposition: accepted"));
        assert!(text.contains("Kept the long block."));
        assert!(text.contains("- trigger: on demand"));
        assert!(text.contains("sha256 "), "inputs are pinned by hash");
    }
}
