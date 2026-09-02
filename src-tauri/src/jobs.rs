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

use crate::ingest::{self, Call};
use crate::prompt::PromptId;
use crate::{agent, ledger, settings, summarise, writer};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

pub struct Prompts {
    pub day_context: String,
    pub ingest_messages: String,
    pub ingest_apps: String,
    pub ingest_websites: String,
}

impl Prompts {
    pub fn load(config_dir: &Path) -> Prompts {
        Prompts {
            day_context: crate::prompt::current(config_dir, PromptId::DayContext),
            ingest_messages: crate::prompt::current(config_dir, PromptId::IngestMessages),
            ingest_apps: crate::prompt::current(config_dir, PromptId::IngestApps),
            ingest_websites: crate::prompt::current(config_dir, PromptId::IngestWebsites),
        }
    }

    pub fn for_call(&self, call: Call) -> &str {
        match call {
            Call::Messages => &self.ingest_messages,
            Call::Apps => &self.ingest_apps,
            Call::Websites => &self.ingest_websites,
        }
    }
}

pub struct Pipeline<'a> {
    pub folder: &'a Path,
    pub summary_agent: &'a settings::Agent,
    pub ingest_agent: &'a settings::Agent,
    pub prompts: &'a Prompts,
    pub ingest_max_chars: usize,
    pub reject_dir: &'a Path,
    pub env: &'a std::collections::HashMap<String, String>,
}

fn timeline_input(folder: &Path, date: NaiveDate) -> Result<(String, ledger::Input), String> {
    let timeline = crate::days::timeline(folder, date)
        .ok_or_else(|| format!("there is no capture for {date}"))?;
    let input = ledger::Input {
        path: PathBuf::from(format!("Days/{}/timeline", date.format("%Y-%m-%d"))),
        sha256: ledger::sha256_of(timeline.as_bytes()),
    };
    Ok((timeline, input))
}

fn call_input(folder: &Path, date: NaiveDate, call: Call) -> Option<String> {
    match call {
        Call::Websites => {
            crate::days::read_day(folder, date, crate::writer::DayFile::Websites)?;
            Some(crate::days::render_totals(&crate::days::website_totals(
                folder, date,
            )))
        }
        other => crate::days::read_day(folder, date, other.source()),
    }
}

pub fn ingest_call(
    p: &Pipeline,
    call: Call,
    date: NaiveDate,
    trigger: ledger::Trigger,
) -> Result<(), String> {
    let (timeline, timeline_input) = timeline_input(p.folder, date)?;
    let template = p.prompts.for_call(call);
    let prompt_sha = ledger::sha256_of(template.as_bytes());

    let Some(raw) = call_input(p.folder, date, call) else {
        ingest::write_skipped(p.folder, date, call).map_err(|e| e.to_string())?;
        return Ok(());
    };
    let input_sha = ledger::sha256_of(raw.as_bytes());
    let timeline_sha = timeline_input.sha256.clone();
    let (input, trimmed) = ingest::trim_input(&raw, p.ingest_max_chars);

    let mut entry = ledger::Entry {
        at: Local::now(),
        trigger,
        action: call.action().to_string(),
        prompt_id: Some(call.prompt().as_str().to_string()),
        prompt_sha256: Some(prompt_sha.clone()),
        engine: Some(p.ingest_agent.label.clone()),
        inputs: vec![
            ledger::Input {
                path: call.source().path(p.folder, date),
                sha256: input_sha.clone(),
            },
            timeline_input,
        ],
        output: None,
        reasoning: if trimmed > 0 {
            Some(format!("input trimmed: {trimmed} blocks"))
        } else {
            None
        },
        disposition: ledger::Disposition::Accepted,
    };
    let record = |disposition: &str| ingest::CallRecord {
        disposition: disposition.to_string(),
        input_sha256: input_sha.clone(),
        timeline_sha256: timeline_sha.clone(),
        prompt_sha256: prompt_sha.clone(),
        engine: p.ingest_agent.label.clone(),
        at: Local::now().to_rfc3339(),
    };

    let prompt = template
        .replace("{{DATE}}", &date.format("%Y-%m-%d").to_string())
        .replace("{{TIMELINE}}", &timeline)
        .replace("{{INPUT}}", &input);

    let output = match agent::run_with_env(p.ingest_agent, &prompt, p.env) {
        Ok(output) => output,
        Err(error) => {
            let message = error.to_string();
            entry.disposition = ledger::Disposition::Failed {
                stderr: message.clone(),
            };
            record_in_ledger(p.folder, &entry);
            let _ = ingest::record_call(p.folder, date, call, record("failed"));
            return Err(message);
        }
    };
    entry.output = Some(output.clone());
    let split = ingest::split_output(&output);
    if let Some(reasoning) = &split.reasoning {
        entry.reasoning = Some(match entry.reasoning.take() {
            Some(prefix) => format!("{prefix}\n\n{reasoning}"),
            None => reasoning.clone(),
        });
    }
    let spans = crate::days::spans(&timeline);
    if let Err(invalid) = ingest::validate(call, &split, &spans) {
        let _ = std::fs::create_dir_all(p.reject_dir);
        let _ = std::fs::write(
            p.reject_dir
                .join(format!("{}-{}.md", date.format("%Y-%m-%d"), call.label())),
            &output,
        );
        entry.disposition = ledger::Disposition::Rejected {
            reason: invalid.to_string(),
        };
        record_in_ledger(p.folder, &entry);
        let _ = ingest::record_call(p.folder, date, call, record("rejected"));
        return Err(format!("{invalid}; the output was kept for inspection"));
    }
    let fm = ingest::Frontmatter {
        date,
        source: call.source().file_name().to_string(),
        generated_by: p.ingest_agent.label.clone(),
        prompt_sha256: prompt_sha.clone(),
    };
    if let Err(error) = ingest::write_call(p.folder, date, call, &split.files, &fm) {
        // The agent's output is the expensive part; keep it in the ledger
        // even though the KB write failed.
        let message = format!("the KB could not be written: {error}");
        entry.disposition = ledger::Disposition::Failed {
            stderr: message.clone(),
        };
        record_in_ledger(p.folder, &entry);
        let _ = ingest::record_call(p.folder, date, call, record("failed"));
        return Err(message);
    }
    ingest::record_call(p.folder, date, call, record("accepted")).map_err(|e| e.to_string())?;
    record_in_ledger(p.folder, &entry);
    Ok(())
}

pub fn summarise_day(
    p: &Pipeline,
    date: NaiveDate,
    trigger: ledger::Trigger,
) -> Result<(), String> {
    let (timeline, timeline_input) = timeline_input(p.folder, date)?;
    let kb = ingest::kb_for_prompt(p.folder, date);
    let mut inputs = vec![
        ledger::hash_file(&writer::DayFile::Apps.path(p.folder, date))
            .map_err(|e| e.to_string())?,
        timeline_input,
    ];
    for name in ingest::KB_FILES {
        if let Ok(input) = ledger::hash_file(&ingest::kb_dir(p.folder, date).join(name)) {
            inputs.push(input);
        }
    }
    let template = &p.prompts.day_context;
    let mut entry = ledger::Entry {
        at: Local::now(),
        trigger,
        action: "summarise_day".to_string(),
        prompt_id: Some("day-context".to_string()),
        prompt_sha256: Some(ledger::sha256_of(template.as_bytes())),
        engine: Some(p.summary_agent.label.clone()),
        inputs,
        output: None,
        reasoning: None,
        disposition: ledger::Disposition::Accepted,
    };

    let prompt = summarise::build_prompt(template, date, &timeline, &kb);

    let output = match agent::run_with_env(p.summary_agent, &prompt, p.env) {
        Ok(output) => output,
        Err(error) => {
            let message = error.to_string();
            entry.disposition = ledger::Disposition::Failed {
                stderr: message.clone(),
            };
            record_in_ledger(p.folder, &entry);
            return Err(message);
        }
    };

    entry.output = Some(output.clone());
    entry.reasoning = summarise::reasoning_of(&output);

    // The evidence is exactly what the model was given: anything the
    // summary states has to be traceable back to one of these two.
    let evidence = format!("{timeline}\n{kb}");
    if let Err(invalid) = summarise::validate(
        &output,
        summarise::MAX_SUMMARY_LINES,
        &crate::days::spans(&timeline),
        &evidence,
    ) {
        let _ = std::fs::create_dir_all(p.reject_dir);
        let _ = std::fs::write(p.reject_dir.join(format!("{date}.md")), &output);
        entry.disposition = ledger::Disposition::Rejected {
            reason: invalid.to_string(),
        };
        record_in_ledger(p.folder, &entry);
        return Err(format!("{invalid}; the output was kept for inspection"));
    }

    summarise::write_summary(p.folder, date, &output)
        .map_err(|e| format!("the summary could not be written: {e}"))?;
    entry.disposition = ledger::Disposition::Accepted;
    record_in_ledger(p.folder, &entry);
    Ok(())
}

pub fn run_day_pipeline(
    p: &Pipeline,
    date: NaiveDate,
    trigger: ledger::Trigger,
    force_ingest: bool,
    summarise: bool,
    mut on_step: impl FnMut(&str),
) -> Result<(), String> {
    let (timeline, _) = timeline_input(p.folder, date)?;
    let timeline_sha = ledger::sha256_of(timeline.as_bytes());
    for (index, call) in Call::ALL.iter().enumerate() {
        let hashes = ingest::Hashes {
            input: call_input(p.folder, date, *call)
                .map(|t| ledger::sha256_of(t.as_bytes()))
                .unwrap_or_else(|| "none".into()),
            timeline: timeline_sha.clone(),
            prompt: ledger::sha256_of(p.prompts.for_call(*call).as_bytes()),
        };
        if !force_ingest && !ingest::needs_ingest(p.folder, date, *call, &hashes) {
            continue;
        }
        if hashes.input != "none" {
            on_step(&format!("ingesting {} ({} of 3)", call.label(), index + 1));
        }
        ingest_call(p, *call, date, trigger.clone())?;
    }
    if summarise {
        on_step("summarising");
        summarise_day(p, date, trigger)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JobKind {
    Summarise,
    Ingest { force: bool },
}

pub fn run_one(
    app: &AppHandle,
    date: NaiveDate,
    trigger: ledger::Trigger,
    kind: JobKind,
    on_step: impl FnMut(&str),
) -> Result<(), String> {
    let config = settings::load(app);
    let folder = config.folder.clone().ok_or("no capture folder is set")?;
    let summary_agent = config.agent.clone().ok_or("no agent is connected")?;
    let ingest_agent = config
        .ingest_agent
        .clone()
        .unwrap_or_else(|| summary_agent.clone());
    let config_dir = settings::config_dir(app);
    let prompts = Prompts::load(&config_dir);
    let reject_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("rejected");
    let env = crate::agent_env(app);
    let p = Pipeline {
        folder: &folder,
        summary_agent: &summary_agent,
        ingest_agent: &ingest_agent,
        prompts: &prompts,
        ingest_max_chars: config.ingest_max_chars,
        reject_dir: &reject_dir,
        env: &env,
    };
    match kind {
        JobKind::Summarise => run_day_pipeline(&p, date, trigger, false, true, on_step),
        JobKind::Ingest { force } => run_day_pipeline(&p, date, trigger, force, false, on_step),
    }
}

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

/// Where the last completed run is remembered between launches. Without
/// it every relaunch looks overdue and fires the backfill a minute in.
pub fn state_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("jobs.json")
}

pub fn read_last_run(data_dir: &Path) -> Option<DateTime<Local>> {
    let raw = std::fs::read_to_string(state_path(data_dir)).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let text = value.get("last_run")?.as_str()?;
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|when| when.with_timezone(&Local))
}

pub fn write_last_run(data_dir: &Path, when: DateTime<Local>) {
    let body = serde_json::json!({ "last_run": when.to_rfc3339() });
    if let Some(parent) = state_path(data_dir).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(error) = std::fs::write(
        state_path(data_dir),
        serde_json::to_string_pretty(&body).unwrap_or_default(),
    ) {
        eprintln!("[jobs] could not record the last run: {error}");
    }
}

impl JobState {
    /// The state a relaunch starts from: whatever the last run was, read
    /// back off disk.
    pub fn with_last_run(last_run: Option<DateTime<Local>>) -> Self {
        let state = Self::default();
        if let Ok(mut inner) = state.inner.lock() {
            inner.last_run = last_run;
        }
        state
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

/// Ticks once a minute. A minute is fine granularity for a daily job and
/// costs nothing; it also means a machine waking at 09:14 catches up within
/// a minute rather than waiting for a precise instant it already missed.
const TICK_SECS: u64 = 60;

pub fn start(app: AppHandle) {
    std::thread::spawn(move || loop {
        // A click on Summarise must not wait up to a minute for the next
        // tick, so the queue wakes this thread when something is pushed.
        app.state::<JobQueue>()
            .wait_for_work(std::time::Duration::from_secs(TICK_SECS));
        tick(&app);
    });
}

fn tick(app: &AppHandle) {
    let state = app.state::<JobState>().inner().clone();
    if state.is_running() {
        return;
    }

    let config = settings::load(app);
    let (Some(folder), Some(_)) = (config.folder.clone(), config.agent.clone()) else {
        // A job queued before the agent or the folder went away must not
        // sit as "queued" forever with the window polling it. Fail it with
        // the reason, which is also what the Day view shows.
        let reason = if config.folder.is_none() {
            "No capture folder is set. Choose one in Setup."
        } else {
            "No agent is connected. Choose one in Settings."
        };
        app.state::<JobQueue>().fail_queued(&state, reason);
        return;
    };

    let pending = due(
        Local::now(),
        config.schedule_hhmm.as_deref(),
        state.last_run(),
        &summarise::list_captured(&folder),
        &summarise::list_summarised(&folder),
    );

    let queued: Vec<QueuedJob> = app.state::<JobQueue>().drain_if_idle(&state);
    // Snapshotted the moment this batch is taken, so a Stop pressed before
    // the snapshot cancels nothing (there is no batch yet) and a Stop
    // pressed after it is what the per-job check below detects.
    let generation = app.state::<JobQueue>().generation();
    if pending.is_empty() && queued.is_empty() {
        return;
    }

    state.set_running(true);
    for date in pending {
        let result = run_one(
            app,
            date,
            ledger::Trigger::Schedule,
            JobKind::Summarise,
            |_| {},
        );
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
        persist_last_run(app);
        if result.is_err() {
            // One failure is usually the agent, and every following day
            // would fail the same way. Stop and let the user see it.
            break;
        }
    }
    // Queued on-demand runs, from the window and from MCP clients named in
    // each job's trigger. A queued failure only stops its own job.
    for job in queued {
        // Checked once per job against the snapshot taken when this batch
        // was drained, so a Stop pressed mid-batch cancels the remainder
        // without touching whatever gets queued after this batch finishes.
        if !should_run(generation, app.state::<JobQueue>().generation()) {
            app.state::<JobQueue>()
                .record(&job.id, JobStatus::Cancelled);
            continue;
        }
        let date = job.date;
        let kind = job.kind;
        app.state::<JobQueue>().record(&job.id, JobStatus::Running);
        let job_id = job.id.clone();
        let result = run_one(app, date, job.trigger, kind, |step| {
            app.state::<JobQueue>().record_step(&job_id, step);
        });
        let status = match &result {
            Ok(()) => JobStatus::Done,
            Err(stderr) => JobStatus::Failed {
                stderr: stderr.clone(),
            },
        };
        app.state::<JobQueue>().record(&job.id, status);
        let outcome = Outcome {
            when: Local::now(),
            date,
            ok: result.is_ok(),
            message: match (&result, kind) {
                (Ok(()), JobKind::Summarise) => format!("Summarised {date}"),
                (Ok(()), JobKind::Ingest { .. }) => format!("Ingested {date}"),
                (Err(message), _) => format!("{date} failed: {message}"),
            },
        };
        state.record(outcome);
        persist_last_run(app);
    }
    state.set_running(false);
    crate::tray::refresh(
        app,
        app.state::<crate::capture::CaptureState>().is_running(),
    );
}

/// Writes the moment of the run just finished, so a relaunch knows the
/// schedule has already been served today.
fn persist_last_run(app: &AppHandle) {
    if let Ok(data_dir) = app.path().app_data_dir() {
        write_last_run(&data_dir, Local::now());
    }
}

/// Whether a job drained into a batch should still run. The runner takes
/// one snapshot of the cancel generation per batch and asks this before
/// each job in it: Stop bumps the generation, so a mismatch means Stop
/// landed after this batch was taken and the rest of it is cancelled.
///
/// Pulled out of `tick` because that needs a live `AppHandle` and cannot be
/// called from a test, which left the decision the Stop button rests on
/// with nothing exercising it.
fn should_run(batch_generation: u64, current_generation: u64) -> bool {
    batch_generation == current_generation
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Done,
    Failed {
        stderr: String,
    },
    /// Skipped because the user pressed Stop. Distinct from Done so the
    /// progress line can say how many never ran.
    Cancelled,
}

/// Identifies one queued run. A newtype over the string the wire carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JobId(pub String);

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct JobSummary {
    pub id: String,
    pub date: NaiveDate,
    pub kind: JobKind,
    pub status: JobStatus,
    pub step: Option<String>,
}

#[derive(Debug)]
struct QueuedJob {
    id: String,
    date: NaiveDate,
    kind: JobKind,
    trigger: ledger::Trigger,
}

/// The on-demand queue. Control and MCP surfaces push into it; the tick
/// drains it when nothing else is running, so agent work stays serial and
/// off the capture thread.
#[derive(Default)]
pub struct JobQueue {
    queue: Mutex<std::collections::VecDeque<QueuedJob>>,
    history: Mutex<Vec<JobSummary>>,
    counter: std::sync::atomic::AtomicU64,
    /// Wakes the runner when a job is pushed, so an on-demand run starts
    /// within about a second rather than at the next tick.
    work: std::sync::Condvar,
    /// Bumped by Stop. A generation rather than a flag because a flag
    /// cannot tell "raised while idle" from "raised for a batch already in
    /// flight": anything else that enqueues while Stop is being handled
    /// would clear a plain flag and let the cancelled batch run anyway. The
    /// runner snapshots this once per batch and compares before each job.
    cancel_generation: std::sync::atomic::AtomicU64,
}

impl JobQueue {
    #[cfg(test)]
    pub fn for_test() -> Self {
        Self::default()
    }

    fn push(&self, date: NaiveDate, kind: JobKind, trigger: ledger::Trigger) -> JobId {
        use std::sync::atomic::Ordering;
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        let id = format!("job-{n}");
        self.queue.lock().expect("job queue").push_back(QueuedJob {
            id: id.clone(),
            date,
            kind,
            trigger,
        });
        self.history.lock().expect("job history").push(JobSummary {
            id: id.clone(),
            date,
            kind,
            status: JobStatus::Queued,
            step: None,
        });
        self.work.notify_all();
        JobId(id)
    }

    /// Blocks until something is queued or the timeout expires, whichever
    /// comes first. The runner's own pacing, so the tick is still a tick
    /// when nothing is asking.
    pub fn wait_for_work(&self, timeout: std::time::Duration) {
        let queue = self.queue.lock().expect("job queue");
        if !queue.is_empty() {
            return;
        }
        let _ = self.work.wait_timeout(queue, timeout).expect("job queue");
    }

    /// The trigger travels with the job because the runner writes the ledger
    /// entry, and an MCP-triggered summary must name the client that asked.
    pub fn enqueue_summarise_with(&self, date: NaiveDate, trigger: ledger::Trigger) -> JobId {
        self.push(date, JobKind::Summarise, trigger)
    }

    /// Queues a full ingest run, optionally forcing all three calls to rerun.
    pub fn enqueue_ingest_with(
        &self,
        date: NaiveDate,
        force: bool,
        trigger: ledger::Trigger,
    ) -> JobId {
        self.push(date, JobKind::Ingest { force }, trigger)
    }

    #[cfg(test)]
    pub fn enqueue_summarise(&self, date: NaiveDate) -> JobId {
        self.enqueue_summarise_with(date, ledger::Trigger::OnDemand)
    }

    /// The eight most recent jobs, newest first. Eight because it is more than
    /// a day's worth of catch-up and short enough to put in a tool result.
    pub fn recent(&self) -> Vec<JobSummary> {
        let history = self.history.lock().expect("job history");
        history.iter().rev().take(8).cloned().collect()
    }

    /// One job by id, queued or finished. The window polls this after
    /// pressing Summarise, and a job that has not started yet must still be
    /// findable.
    pub fn find(&self, id: &str) -> Option<JobSummary> {
        let history = self.history.lock().expect("job history");
        history.iter().find(|job| job.id == id).cloned()
    }

    /// The ids of every job still queued or running, oldest first. The
    /// window adopts these on mount: the runner outlives the webview, so a
    /// window closed mid-batch and reopened would otherwise show no batch
    /// at all, with Stop disabled and Summarise offering to enqueue days
    /// that are already on their way.
    pub fn outstanding(&self) -> Vec<String> {
        let history = self.history.lock().expect("job history");
        history
            .iter()
            .filter(|job| matches!(job.status, JobStatus::Queued | JobStatus::Running))
            .map(|job| job.id.clone())
            .collect()
    }

    /// Takes every queued job when nothing is running, and nothing at all
    /// when a run is in flight: that is what keeps on-demand and scheduled
    /// runs serial. The queue is left intact for the next tick.
    fn drain_if_idle(&self, state: &JobState) -> Vec<QueuedJob> {
        if state.is_running() {
            return Vec::new();
        }
        let mut queue = self.queue.lock().expect("job queue");
        std::mem::take(&mut *queue).into_iter().collect()
    }

    fn record(&self, id: &str, status: JobStatus) {
        let mut history = self.history.lock().expect("job history");
        if let Some(job) = history.iter_mut().find(|job| job.id == id) {
            job.status = status;
            if matches!(
                job.status,
                JobStatus::Done | JobStatus::Failed { .. } | JobStatus::Cancelled
            ) {
                job.step = None;
            }
        }
    }

    pub fn record_step(&self, id: &str, step: &str) {
        let mut history = self.history.lock().expect("job history");
        if let Some(job) = history.iter_mut().find(|job| job.id == id) {
            job.step = Some(step.to_string());
        }
    }

    /// Empties the queue, marks what it took as cancelled, and bumps the
    /// generation so the runner drops whatever it already drained. Returns
    /// how many it cleared, which is not the whole story: the runner may
    /// hold more. The caller counts its own jobs to report a total.
    pub fn cancel_queued(&self) -> usize {
        use std::sync::atomic::Ordering;
        self.cancel_generation.fetch_add(1, Ordering::SeqCst);
        let dropped: Vec<QueuedJob> = self.queue.lock().expect("job queue").drain(..).collect();
        for job in &dropped {
            self.record(&job.id, JobStatus::Cancelled);
        }
        dropped.len()
    }

    /// The current cancel generation. The runner snapshots this once per
    /// batch, right after draining it, and compares before each job: a
    /// mismatch means Stop was pressed after the snapshot was taken, so
    /// this batch (and only this batch) should stop running.
    pub fn generation(&self) -> u64 {
        use std::sync::atomic::Ordering;
        self.cancel_generation.load(Ordering::SeqCst)
    }

    /// Drains everything queued and marks it failed with `reason`. Used when
    /// the tick finds nothing that could run a job, so nothing is left in
    /// "queued" with no path out of it. Returns how many were failed.
    pub(crate) fn fail_queued(&self, state: &JobState, reason: &str) -> usize {
        let drained = self.drain_if_idle(state);
        for job in &drained {
            self.record(
                &job.id,
                JobStatus::Failed {
                    stderr: reason.to_string(),
                },
            );
        }
        drained.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Local, NaiveDate, TimeZone};

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
        assert!(due(
            at(2026, 8, 29, 9, 0),
            Some("nonsense"),
            None,
            &captured,
            &[]
        )
        .is_empty());
    }

    use tempfile::tempdir;

    /// A stub agent is an absolute path, so the environment only has to
    /// be a plausible one rather than the user's own.
    fn test_env() -> std::collections::HashMap<String, String> {
        std::collections::HashMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())])
    }

    fn stub_agent(command: &str, args: &[&str]) -> crate::settings::Agent {
        crate::settings::Agent {
            label: "stub".to_string(),
            command: command.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
            timeout_secs: 10,
        }
    }

    fn stub_reply(dir: &std::path::Path, name: &str, reply: &str) -> crate::settings::Agent {
        let path = dir.join(name);
        std::fs::write(&path, reply).unwrap();
        stub_agent(
            "/bin/sh",
            &["-c", &format!("cat > /dev/null; cat '{}'", path.display())],
        )
    }

    fn write_days(folder: &std::path::Path) {
        use crate::writer::DayFile;
        let d = day(2026, 8, 28);
        for (file, text) in [
            (
                DayFile::Apps,
                "---\ndate: 2026-08-28\nkind: apps\n---\n\n## 09:00\u{2013}11:00 \u{00b7} Zed \u{b7} jobs.rs\n\nfile: /x/jobs.rs\n\nfn tick\n\n## 11:00\u{2013}11:20 \u{00b7} Slack \u{b7} #x\n\nrouted: messages\n",
            ),
            (
                DayFile::Messages,
                "---\ndate: 2026-08-28\nkind: messages\n---\n\n## 11:00\u{2013}11:20 \u{00b7} Slack \u{b7} #x\n\ndan: can you ship it thursday\n",
            ),
            (
                DayFile::Websites,
                "---\ndate: 2026-08-28\nkind: websites\n---\n\n| start | end | app | domain | title | url |\n| --- | --- | --- | --- | --- | --- |\n| 09:30 | 09:41 | Arc | v2.tauri.app | Tauri | https://v2.tauri.app/ |\n",
            ),
        ] {
            let path = file.path(folder, d);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
    }

    const APPS_REPLY: &str = "<<<file: threads.md>>>\n## jobs.rs\nEdited tick 09:00-11:00 file: /x/jobs.rs\n<<<file: products.md>>>\nNothing evident.\n<<<file: issues.md>>>\nNothing evident.\n<<<reasoning>>>\nOne thread.\n";
    const MESSAGES_REPLY: &str = "<<<file: people.md>>>\n## Dan\nAsked to ship it Thursday 11:00-11:20\n<<<file: commitments.md>>>\n## I agreed to\n- [ ] ship it · with Dan · 11:00-11:20 · none\n\n## Owed to me\nNothing evident.\n";
    const WEBSITES_REPLY: &str = "<<<file: reading.md>>>\nNothing evident.\n";

    fn prompts() -> Prompts {
        Prompts {
            day_context: "{{DATE}}\n{{TIMELINE}}\n{{KB}}".into(),
            ingest_messages: "{{DATE}} {{TIMELINE}} {{INPUT}}".into(),
            ingest_apps: "{{DATE}} {{TIMELINE}} {{INPUT}}".into(),
            ingest_websites: "{{DATE}} {{TIMELINE}} {{INPUT}}".into(),
        }
    }

    fn pipeline<'a>(
        folder: &'a std::path::Path,
        rejects: &'a std::path::Path,
        agent: &'a crate::settings::Agent,
        prompts: &'a Prompts,
        env: &'a std::collections::HashMap<String, String>,
    ) -> Pipeline<'a> {
        Pipeline {
            folder,
            summary_agent: agent,
            ingest_agent: agent,
            prompts,
            ingest_max_chars: 400_000,
            reject_dir: rejects,
            env,
        }
    }

    fn write_fixture(dir: &std::path::Path, name: &str, text: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        path
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
    fn an_accepted_ingest_call_writes_its_files_manifest_and_ledger() {
        let folder = tempdir().unwrap();
        let rejects = tempdir().unwrap();
        write_days(folder.path());
        let agent = stub_reply(folder.path(), "reply.md", APPS_REPLY);
        let prompts = prompts();
        let env = test_env();
        let p = pipeline(folder.path(), rejects.path(), &agent, &prompts, &env);
        ingest_call(
            &p,
            Call::Apps,
            day(2026, 8, 28),
            crate::ledger::Trigger::OnDemand,
        )
        .unwrap();
        let kb = ingest::kb_dir(folder.path(), day(2026, 8, 28));
        assert!(kb.join("threads.md").exists());
        assert!(std::fs::read_to_string(kb.join("products.md"))
            .unwrap()
            .contains("generated_by: stub"));
        assert_eq!(
            ingest::read_manifest(folder.path(), day(2026, 8, 28)).calls["ingest_apps"].disposition,
            "accepted"
        );
        let ledger = std::fs::read_to_string(crate::ledger::ledger_path(
            folder.path(),
            Local::now().date_naive(),
        ))
        .unwrap();
        assert!(ledger.contains("ingest_apps"));
        assert!(ledger.contains("Days/2026-08-28/timeline"));
        assert!(ledger.contains("One thread."));
    }

    #[test]
    fn a_rejected_ingest_call_keeps_the_output_and_writes_no_kb_file() {
        let folder = tempdir().unwrap();
        let rejects = tempdir().unwrap();
        write_days(folder.path());
        let agent = stub_reply(
            folder.path(),
            "reply.md",
            "<<<file: threads.md>>>\n## jobs.rs\nno citation here\n<<<file: products.md>>>\nNothing evident.\n<<<file: issues.md>>>\nNothing evident.\n",
        );
        let prompts = prompts();
        let env = test_env();
        let p = pipeline(folder.path(), rejects.path(), &agent, &prompts, &env);
        let error = ingest_call(
            &p,
            Call::Apps,
            day(2026, 8, 28),
            crate::ledger::Trigger::OnDemand,
        )
        .unwrap_err();
        assert!(error.contains("no time range"), "{error}");
        assert!(rejects.path().join("2026-08-28-apps.md").exists());
        assert!(!ingest::kb_dir(folder.path(), day(2026, 8, 28))
            .join("threads.md")
            .exists());
        assert_eq!(
            ingest::read_manifest(folder.path(), day(2026, 8, 28)).calls["ingest_apps"].disposition,
            "rejected"
        );
    }

    #[test]
    fn the_pipeline_runs_three_calls_then_the_summary_and_skips_what_is_current() {
        let folder = tempdir().unwrap();
        let rejects = tempdir().unwrap();
        write_days(folder.path());
        let script = format!(
            "input=$(cat); case \"$input\" in *'dan: can you'*) cat '{m}';; *'fn tick'*) cat '{a}';; *'| domain |'*) cat '{w}';; *) cat '{s}';; esac",
            m = write_fixture(folder.path(), "m.md", MESSAGES_REPLY).display(),
            a = write_fixture(folder.path(), "a.md", APPS_REPLY).display(),
            w = write_fixture(folder.path(), "w.md", WEBSITES_REPLY).display(),
            s = write_fixture(folder.path(), "s.md", &valid_summary()).display(),
        );
        let agent = stub_agent("/bin/sh", &["-c", &script]);
        let prompts = prompts();
        let env = test_env();
        let p = pipeline(folder.path(), rejects.path(), &agent, &prompts, &env);
        let mut steps: Vec<String> = Vec::new();
        run_day_pipeline(
            &p,
            day(2026, 8, 28),
            crate::ledger::Trigger::OnDemand,
            false,
            true,
            |s| steps.push(s.to_string()),
        )
        .unwrap();
        assert_eq!(
            steps,
            vec![
                "ingesting messages (1 of 3)".to_string(),
                "ingesting apps (2 of 3)".to_string(),
                "ingesting websites (3 of 3)".to_string(),
                "summarising".to_string(),
            ]
        );
        assert!(crate::summarise::summary_path(folder.path(), day(2026, 8, 28)).exists());
        let ledger = std::fs::read_to_string(crate::ledger::ledger_path(
            folder.path(),
            Local::now().date_naive(),
        ))
        .unwrap();
        for action in [
            "ingest_messages",
            "ingest_apps",
            "ingest_websites",
            "summarise_day",
        ] {
            assert!(ledger.contains(action), "{action}");
        }
        assert!(
            ledger.contains("KB/2026-08-28/people.md"),
            "summary inputs list KB files"
        );

        steps.clear();
        run_day_pipeline(
            &p,
            day(2026, 8, 28),
            crate::ledger::Trigger::OnDemand,
            false,
            true,
            |s| steps.push(s.to_string()),
        )
        .unwrap();
        assert_eq!(steps, vec!["summarising".to_string()]);

        steps.clear();
        run_day_pipeline(
            &p,
            day(2026, 8, 28),
            crate::ledger::Trigger::OnDemand,
            true,
            false,
            |s| steps.push(s.to_string()),
        )
        .unwrap();
        assert_eq!(steps.len(), 3);
    }

    #[test]
    fn a_day_without_messages_skips_that_call_and_writes_nothing_evident() {
        let folder = tempdir().unwrap();
        let rejects = tempdir().unwrap();
        write_days(folder.path());
        std::fs::remove_file(
            crate::writer::DayFile::Messages.path(folder.path(), day(2026, 8, 28)),
        )
        .unwrap();
        let script = format!(
            "input=$(cat); case \"$input\" in *'fn tick'*) cat '{a}';; *) cat '{w}';; esac",
            a = write_fixture(folder.path(), "a.md", APPS_REPLY).display(),
            w = write_fixture(folder.path(), "w.md", WEBSITES_REPLY).display(),
        );
        let agent = stub_agent("/bin/sh", &["-c", &script]);
        let prompts = prompts();
        let env = test_env();
        let p = pipeline(folder.path(), rejects.path(), &agent, &prompts, &env);
        let mut steps: Vec<String> = Vec::new();
        run_day_pipeline(
            &p,
            day(2026, 8, 28),
            crate::ledger::Trigger::OnDemand,
            false,
            false,
            |s| steps.push(s.to_string()),
        )
        .unwrap();
        assert_eq!(
            steps,
            vec![
                "ingesting apps (2 of 3)".to_string(),
                "ingesting websites (3 of 3)".to_string(),
            ]
        );
        let manifest = ingest::read_manifest(folder.path(), day(2026, 8, 28));
        assert_eq!(manifest.calls["ingest_messages"].disposition, "skipped");
        assert!(std::fs::read_to_string(
            ingest::kb_dir(folder.path(), day(2026, 8, 28)).join("people.md")
        )
        .unwrap()
        .contains("Nothing evident."));
    }

    #[test]
    fn a_failed_ingest_stops_the_pipeline_before_the_summary() {
        let folder = tempdir().unwrap();
        let rejects = tempdir().unwrap();
        write_days(folder.path());
        let agent = stub_agent("/bin/sh", &["-c", "echo not logged in >&2; exit 1"]);
        let prompts = prompts();
        let env = test_env();
        let p = pipeline(folder.path(), rejects.path(), &agent, &prompts, &env);
        let error = run_day_pipeline(
            &p,
            day(2026, 8, 28),
            crate::ledger::Trigger::Schedule,
            false,
            true,
            |_| {},
        )
        .unwrap_err();
        assert!(error.contains("not logged in"), "{error}");
        assert!(!crate::summarise::summary_path(folder.path(), day(2026, 8, 28)).exists());
    }

    #[test]
    fn a_malformed_run_writes_one_ledger_entry_and_no_summary() {
        let folder = tempdir().unwrap();
        let rejects = tempdir().unwrap();
        write_days(folder.path());

        let agent = stub_agent("/bin/echo", &["not a summary"]);
        let prompts = prompts();
        let env = test_env();
        let p = pipeline(folder.path(), rejects.path(), &agent, &prompts, &env);
        let error =
            summarise_day(&p, day(2026, 8, 28), crate::ledger::Trigger::Schedule).unwrap_err();

        assert!(error.contains("frontmatter"), "error was {error:?}");
        assert!(!crate::summarise::summary_path(folder.path(), day(2026, 8, 28)).exists());
        assert!(rejects.path().join("2026-08-28.md").exists());

        let ledger_file = crate::ledger::ledger_path(folder.path(), Local::now().date_naive());
        let text = std::fs::read_to_string(ledger_file).unwrap();
        assert_eq!(text.matches("\n## ").count(), 1, "exactly one entry");
        assert!(text.contains("rejected:"));
    }

    #[test]
    fn an_agent_that_never_returns_an_answer_is_still_ledgered() {
        let folder = tempdir().unwrap();
        let rejects = tempdir().unwrap();
        write_days(folder.path());

        let agent = stub_agent("/bin/sh", &["-c", "echo not logged in >&2; exit 1"]);
        let prompts = prompts();
        let env = test_env();
        let p = pipeline(folder.path(), rejects.path(), &agent, &prompts, &env);
        let error =
            summarise_day(&p, day(2026, 8, 28), crate::ledger::Trigger::Schedule).unwrap_err();

        assert!(error.contains("not logged in"), "error was {error:?}");
        let ledger_file = crate::ledger::ledger_path(folder.path(), Local::now().date_naive());
        let text = std::fs::read_to_string(ledger_file).unwrap();
        assert_eq!(text.matches("\n## ").count(), 1);
        assert!(text.contains("failed:"));
    }

    #[test]
    fn a_valid_run_writes_the_summary_and_an_accepted_entry_with_its_reasoning() {
        let folder = tempdir().unwrap();
        let rejects = tempdir().unwrap();
        write_days(folder.path());

        let agent = stub_agent("/bin/cat", &[]);
        let prompts = Prompts {
            day_context: valid_summary(),
            ingest_messages: String::new(),
            ingest_apps: String::new(),
            ingest_websites: String::new(),
        };
        let env = test_env();
        let p = pipeline(folder.path(), rejects.path(), &agent, &prompts, &env);
        summarise_day(&p, day(2026, 8, 28), crate::ledger::Trigger::OnDemand).unwrap();

        let summary = std::fs::read_to_string(crate::summarise::summary_path(
            folder.path(),
            day(2026, 8, 28),
        ))
        .unwrap();
        assert!(summary.contains("# A day of plumbing"));

        let ledger_file = crate::ledger::ledger_path(folder.path(), Local::now().date_naive());
        let text = std::fs::read_to_string(ledger_file).unwrap();
        assert!(text.contains("- disposition: accepted"));
        assert!(text.contains("Kept the long block."));
        assert!(text.contains("- trigger: on demand"));
        assert!(text.contains("sha256 "), "inputs are pinned by hash");
    }

    #[test]
    fn ingest_jobs_can_be_enqueued() {
        let queue = JobQueue::for_test();
        let id = queue.enqueue_ingest_with(day(2026, 8, 30), false, ledger::Trigger::OnDemand);
        let job = queue.find(&id.0).expect("the queued job");
        assert!(matches!(job.kind, JobKind::Ingest { force: false }));
    }

    #[test]
    fn the_bare_enqueue_records_an_on_demand_trigger() {
        let queue = JobQueue::for_test();
        let id = queue.enqueue_summarise(chrono::NaiveDate::from_ymd_opt(2026, 8, 30).unwrap());
        let recent = queue.recent();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, id.to_string());
    }

    #[test]
    fn a_queued_job_waits_while_a_run_is_in_flight() {
        // The on-demand path goes through the queue precisely so that a
        // click cannot start a second agent run beside a scheduled one.
        let queue = JobQueue::for_test();
        let state = JobState::default();
        queue.enqueue_summarise(day(2026, 8, 30));

        state.set_running(true);
        assert!(queue.drain_if_idle(&state).is_empty(), "nothing runs yet");

        state.set_running(false);
        let drained = queue.drain_if_idle(&state);
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].date, day(2026, 8, 30));
    }

    #[test]
    fn a_job_is_findable_by_id_while_it_is_still_queued() {
        let queue = JobQueue::for_test();
        let id = queue.enqueue_summarise(day(2026, 8, 30));
        let found = queue.find(&id.to_string()).expect("the queued job");
        assert_eq!(found.status, JobStatus::Queued);
        assert_eq!(found.date, day(2026, 8, 30));
        assert!(queue.find("job-nope").is_none());
    }

    #[test]
    fn a_queued_job_with_nothing_to_run_it_is_failed_not_stranded() {
        let queue = JobQueue::for_test();
        let state = JobState::default();
        let id = queue.enqueue_summarise(chrono::NaiveDate::from_ymd_opt(2026, 8, 30).unwrap());
        assert_eq!(queue.fail_queued(&state, "No agent is connected."), 1);
        let job = queue.find(&id.0).expect("the job stays findable");
        assert!(
            matches!(job.status, JobStatus::Failed { ref stderr } if stderr.contains("No agent"))
        );
        assert_eq!(
            queue.fail_queued(&state, "again"),
            0,
            "nothing left to fail"
        );
    }

    #[test]
    fn pushing_a_job_wakes_the_runner_rather_than_letting_it_sleep_out_the_tick() {
        let queue = std::sync::Arc::new(JobQueue::for_test());
        let pusher = queue.clone();
        let started = std::time::Instant::now();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            pusher.enqueue_summarise(day(2026, 8, 30));
        });
        queue.wait_for_work(std::time::Duration::from_secs(30));
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "the wait returned in {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn the_last_run_survives_a_relaunch() {
        let dir = tempdir().unwrap();
        assert!(read_last_run(dir.path()).is_none());
        let when = at(2026, 8, 29, 6, 0);
        write_last_run(dir.path(), when);
        let state = JobState::with_last_run(read_last_run(dir.path()));
        assert_eq!(state.last_run(), Some(when));
    }

    #[test]
    fn recent_returns_the_newest_first_and_caps_at_eight() {
        let queue = JobQueue::for_test();
        for day in 1..=10 {
            queue.enqueue_summarise(chrono::NaiveDate::from_ymd_opt(2026, 8, day).unwrap());
        }
        let recent = queue.recent();
        assert_eq!(recent.len(), 8);
        assert_eq!(recent[0].date.day(), 10);
    }

    #[test]
    fn cancelling_empties_the_queue_and_marks_those_jobs() {
        let queue = JobQueue::for_test();
        let a = queue.enqueue_summarise(day(2026, 8, 28));
        let b = queue.enqueue_summarise(day(2026, 8, 29));

        assert_eq!(queue.cancel_queued(), 2);

        assert_eq!(queue.find(&a.0).unwrap().status, JobStatus::Cancelled);
        assert_eq!(queue.find(&b.0).unwrap().status, JobStatus::Cancelled);
    }

    #[test]
    fn cancelling_while_idle_does_not_kill_the_next_batch() {
        let queue = JobQueue::for_test();
        queue.cancel_queued();

        // A batch drained after this point snapshots the generation the
        // cancel already bumped to, so comparing against its own snapshot
        // later finds no mismatch and nothing in it gets cancelled.
        let next = queue.enqueue_summarise(day(2026, 8, 30));
        let generation = queue.generation();
        assert!(should_run(generation, queue.generation()));
        assert_eq!(queue.find(&next.0).unwrap().status, JobStatus::Queued);
    }

    #[test]
    fn generation_only_changes_on_cancel() {
        let queue = JobQueue::for_test();
        queue.enqueue_summarise(day(2026, 8, 28));
        let before = queue.generation();

        queue.cancel_queued();

        let after = queue.generation();
        assert_ne!(before, after);
        // Reading it again does not consume or change it, unlike the flag
        // this replaced: the runner reads it once per job for the whole
        // batch, not once in total.
        assert_eq!(queue.generation(), after);
    }

    #[test]
    fn cancelling_mid_batch_is_not_undone_by_a_later_enqueue() {
        // Reproduces the bug in the flag-based design: Stop pressed after a
        // batch is already drained must not be erased by something else
        // (an MCP client, the Day view's own Summarise button) enqueuing
        // while the cancelled batch is still winding down.
        let queue = JobQueue::for_test();
        let state = JobState::default();
        queue.enqueue_summarise(day(2026, 8, 28));
        queue.enqueue_summarise(day(2026, 8, 29));

        let _drained = queue.drain_if_idle(&state);
        let generation = queue.generation(); // the runner's snapshot for this batch

        assert_eq!(queue.cancel_queued(), 0); // Stop; the queue is already empty

        // Something else enqueues while the cancelled batch is still in flight.
        queue.enqueue_summarise(day(2026, 8, 30));

        // The in-flight batch's snapshot must still show a mismatch.
        assert!(!should_run(generation, queue.generation()));
    }

    #[test]
    fn a_stop_mid_batch_stops_the_runner_at_the_next_job_boundary() {
        // The runner's own loop over a drained batch, with the agent work
        // left out: what is under test is the decision `tick` makes before
        // each job, which is the whole of what Stop does to a batch already
        // taken off the queue.
        let queue = JobQueue::for_test();
        let state = JobState::default();
        let first = queue.enqueue_summarise(day(2026, 8, 28));
        let second = queue.enqueue_summarise(day(2026, 8, 29));
        let third = queue.enqueue_summarise(day(2026, 8, 30));

        let batch = queue.drain_if_idle(&state);
        let generation = queue.generation();

        let mut ran = Vec::new();
        for job in &batch {
            if !should_run(generation, queue.generation()) {
                queue.record(&job.id, JobStatus::Cancelled);
                continue;
            }
            queue.record(&job.id, JobStatus::Running);
            ran.push(job.date);
            queue.record(&job.id, JobStatus::Done);
            // Stop, pressed while the first day was being summarised.
            if job.date == day(2026, 8, 28) {
                queue.cancel_queued();
            }
        }

        assert_eq!(ran, vec![day(2026, 8, 28)]);
        assert_eq!(queue.find(&first.0).unwrap().status, JobStatus::Done);
        assert_eq!(queue.find(&second.0).unwrap().status, JobStatus::Cancelled);
        assert_eq!(queue.find(&third.0).unwrap().status, JobStatus::Cancelled);
    }

    #[test]
    fn a_batch_with_no_stop_runs_every_job_in_it() {
        let queue = JobQueue::for_test();
        let state = JobState::default();
        queue.enqueue_summarise(day(2026, 8, 28));
        queue.enqueue_summarise(day(2026, 8, 29));

        let batch = queue.drain_if_idle(&state);
        let generation = queue.generation();

        // Something else queues a job while this batch runs. That is not a
        // Stop and must not cancel anything.
        queue.enqueue_summarise(day(2026, 8, 31));

        let ran = batch
            .iter()
            .filter(|_| should_run(generation, queue.generation()))
            .count();
        assert_eq!(ran, 2);
    }

    #[test]
    fn outstanding_is_what_is_queued_or_running_and_nothing_else() {
        let queue = JobQueue::for_test();
        let running = queue.enqueue_summarise(day(2026, 8, 28));
        let done = queue.enqueue_summarise(day(2026, 8, 29));
        let cancelled = queue.enqueue_summarise(day(2026, 8, 30));
        let still_queued = queue.enqueue_summarise(day(2026, 8, 31));
        queue.record(&running.0, JobStatus::Running);
        queue.record(&done.0, JobStatus::Done);
        queue.record(&cancelled.0, JobStatus::Cancelled);

        assert_eq!(queue.outstanding(), vec![running.0, still_queued.0]);
    }
}
