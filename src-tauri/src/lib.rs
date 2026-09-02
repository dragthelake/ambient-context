mod agent;
mod capture;
mod control;
mod days;
mod ingest;
mod ipc;
mod jobs;
mod ledger;
pub mod mcp;
mod prompt;
mod propose;
mod prune;
mod reader;
mod redact;
mod replay;
mod route;
mod rules;
mod segment;
mod settings;
mod summarise;
mod tray;
mod window_chrome;
mod writer;

use serde::Serialize;
use std::os::unix::fs::PermissionsExt;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;

#[tauri::command]
fn permission_status() -> String {
    match reader::macos::permission_status() {
        reader::Permission::Granted => "granted".to_string(),
        reader::Permission::NotGranted => "notGranted".to_string(),
    }
}

#[tauri::command]
fn request_permission() -> String {
    reader::macos::request_permission();
    permission_status()
}

#[tauri::command]
fn current_folder(app: tauri::AppHandle) -> Option<String> {
    settings::load(&app)
        .folder
        .map(|p| p.to_string_lossy().to_string())
}

// Async so it runs on the async runtime's thread pool rather than the main
// thread: blocking_pick_folder parks its calling thread while the dialog
// itself needs the main thread to run.
#[tauri::command]
async fn choose_folder(app: tauri::AppHandle) -> Option<String> {
    let picked = app.dialog().file().blocking_pick_folder()?;
    let path = picked.into_path().ok()?;
    let mut config = settings::load(&app);
    config.folder = Some(path.clone());
    if let Err(error) = save_settings_recorded(
        &settings::config_dir(&app),
        Some(&path),
        "choose_folder",
        &config,
    ) {
        eprintln!("[settings] could not save the chosen folder: {error}");
    }
    Some(path.to_string_lossy().to_string())
}

/// The default is ~/Documents/Ambient Context, where people expect their
/// own files to live. On a Mac with iCloud's Desktop & Documents sync on,
/// that means the record is uploaded; the settings and setup screens carry
/// that warning rather than this path avoiding it.
#[tauri::command]
fn use_default_folder(app: tauri::AppHandle) -> Option<String> {
    let documents = app
        .path()
        .document_dir()
        .or_else(|_| app.path().home_dir())
        .ok()?;
    let path = documents.join("Ambient Context");
    // Create it now, not at first write: Reveal Folder and Open Today's File
    // are dead until the folder exists, which reads as a broken app.
    std::fs::create_dir_all(&path).ok()?;
    let mut config = settings::load(&app);
    config.folder = Some(path.clone());
    if let Err(error) = save_settings_recorded(
        &settings::config_dir(&app),
        Some(&path),
        "use_default_folder",
        &config,
    ) {
        eprintln!("[settings] could not save the default folder: {error}");
    }
    Some(path.to_string_lossy().to_string())
}

#[derive(Serialize)]
struct CaptureStatus {
    running: bool,
    blocks_today: usize,
}

#[tauri::command]
fn capture_status(state: tauri::State<capture::CaptureState>) -> CaptureStatus {
    CaptureStatus {
        running: state.is_running(),
        blocks_today: state.blocks_today(),
    }
}

/// Starts capture if the app just became ready and nothing has told it not
/// to: called by the settings page when the permission grant arrives while
/// the app is already running, where the launch-time auto-start cannot help.
#[tauri::command]
fn start_if_enabled(app: tauri::AppHandle) -> CaptureStatus {
    let state = app.state::<capture::CaptureState>().inner().clone();
    let config = settings::load(&app);
    if !state.is_running()
        && config.enabled
        && config.folder.is_some()
        && reader::macos::permission_status() == reader::Permission::Granted
    {
        capture::start(app.clone(), &state, config);
        tray::refresh(&app, true);
    }
    CaptureStatus {
        running: state.is_running(),
        blocks_today: state.blocks_today(),
    }
}

#[tauri::command]
fn toggle_capture(app: tauri::AppHandle) -> CaptureStatus {
    tray::toggle_capture(&app);
    let state = app.state::<capture::CaptureState>();
    CaptureStatus {
        running: state.is_running(),
        blocks_today: state.blocks_today(),
    }
}

#[derive(Serialize)]
struct CensusSnapshot {
    app: String,
    window_title: Option<String>,
    element_count: usize,
    character_count: usize,
    walk_ms: u128,
    sample: String,
    text: Vec<String>,
}

#[tauri::command]
fn census_snapshot() -> Option<CensusSnapshot> {
    let started = std::time::Instant::now();
    let snap = reader::macos::snapshot()?;
    let character_count = snap.text.iter().map(|line| line.chars().count()).sum();
    let sample: String = snap.text.join(" ").chars().take(200).collect();
    Some(CensusSnapshot {
        app: snap.app,
        window_title: snap.window_title,
        element_count: snap.text.len(),
        character_count,
        walk_ms: started.elapsed().as_millis(),
        sample,
        text: snap.text,
    })
}

/// Opens a link in the user's default browser. The webview itself must
/// never navigate away from the settings page.
#[tauri::command]
fn open_link(url: String) {
    if url.starts_with("https://") {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
}

fn parse_date(date: &str) -> Result<chrono::NaiveDate, String> {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| format!("{date} is not a date"))
}

fn parse_day_file(file: Option<String>) -> Result<writer::DayFile, String> {
    match file.as_deref() {
        None => Ok(writer::DayFile::Apps),
        Some(name) => writer::DayFile::from_name(name)
            .ok_or_else(|| format!("{name} is not one of apps, websites or messages")),
    }
}

/// Which file a Day view action means. Unknown values return None rather
/// than defaulting, so a typo opens nothing instead of the wrong file.
fn target_path(
    folder: &std::path::Path,
    date: chrono::NaiveDate,
    which: &str,
) -> Option<std::path::PathBuf> {
    match which {
        "apps" | "websites" | "messages" => {
            writer::DayFile::from_name(which).map(|f| f.path(folder, date))
        }
        "summary" => Some(summarise::summary_path(folder, date)),
        "kb" => Some(crate::ingest::kb_dir(folder, date).join("threads.md")),
        _ => None,
    }
}

/// Opens one file with the configured editor. No editor configured means
/// the system handler for markdown, which is whatever the user already
/// double-clicks these files with.
fn open_path_in_editor(app: &tauri::AppHandle, path: &std::path::Path) -> Result<(), String> {
    let mut command = std::process::Command::new("open");
    if let Some(editor) = settings::load(app).editor {
        command.arg("-a").arg(editor);
    }
    command.arg(path).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_in_editor(app: tauri::AppHandle, date: String, which: String) -> Result<(), String> {
    let folder = settings::load(&app)
        .folder
        .ok_or("no capture folder is set")?;
    let parsed = parse_date(&date)?;
    let path = target_path(&folder, parsed, &which).ok_or("there is no such file to open")?;
    if !path.is_file() {
        return Err(format!("there is no {which} file for {date} yet"));
    }
    open_path_in_editor(&app, &path)
}

/// Which file "open the prompt in my editor" means. A customised prompt is
/// the user's own file and opens directly. With no customised prompt there
/// is no file to open: writing the bundled text to the prompt path would
/// make `is_customised` true without the user having changed anything, and
/// the Settings panel would then say they have their own prompt when they
/// do not. So the bundled text is copied to a read-only file outside the
/// config directory instead, which is readable and copyable but cannot be
/// edited into place. Editing the prompt is done in Settings, which writes
/// through `set_prompt`.
fn prompt_editor_target(
    config_dir: &std::path::Path,
    temp_dir: &std::path::Path,
    id: prompt::PromptId,
) -> std::io::Result<std::path::PathBuf> {
    if prompt::is_customised(config_dir, id) {
        return Ok(prompt::prompt_path(config_dir, id));
    }
    let copy = temp_dir.join(format!(
        "Ambient Context {} (bundled, read only).md",
        id.as_str()
    ));
    if copy.exists() {
        // A copy from a previous open is read-only, so make it writable
        // before replacing it.
        let _ = std::fs::set_permissions(&copy, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::write(&copy, id.bundled())?;
    std::fs::set_permissions(&copy, std::fs::Permissions::from_mode(0o444))?;
    Ok(copy)
}

/// Opens a prompt in the user's editor. Writes nothing the user did not
/// already have, so there is no ledger entry.
#[tauri::command]
fn open_prompt_in_editor(app: tauri::AppHandle, id: Option<String>) -> Result<(), String> {
    let prompt_id = prompt_id(id)?;
    let path = prompt_editor_target(
        &settings::config_dir(&app),
        &std::env::temp_dir(),
        prompt_id,
    )
    .map_err(|e| e.to_string())?;
    open_path_in_editor(&app, &path)
}

fn prompt_id(id: Option<String>) -> Result<prompt::PromptId, String> {
    match id {
        None => Ok(prompt::PromptId::DayContext),
        Some(name) => {
            prompt::PromptId::parse(&name).ok_or_else(|| format!("{name} is not a prompt id"))
        }
    }
}

#[tauri::command]
fn reveal_day(app: tauri::AppHandle, date: String) -> Result<(), String> {
    let folder = settings::load(&app)
        .folder
        .ok_or("no capture folder is set")?;
    let parsed = parse_date(&date)?;
    let path = writer::day_dir(&folder, parsed);
    // -R selects the file in Finder. A day with no file yet still has a
    // folder worth opening.
    let target = if path.is_dir() { path } else { folder };
    std::process::Command::new("open")
        .arg(target)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(serde::Serialize)]
struct RulesPayload {
    rules: Vec<rules::Rule>,
    built_ins: Vec<rules::BuiltIn>,
    next_id: String,
    /// Why rules.json could not be read, when it could not. The list is
    /// empty in that case, and it is not empty because the user has no
    /// rules: Settings has to be able to tell those two apart.
    error: Option<String>,
    /// Where the file lives, so an error message can point at it.
    path: String,
}

fn rules_payload(config_dir: &std::path::Path) -> RulesPayload {
    let path = rules::rules_path(config_dir).to_string_lossy().to_string();
    match rules::load_result(config_dir) {
        Ok(loaded) => RulesPayload {
            next_id: rules::new_id(&loaded),
            rules: loaded.rules,
            built_ins: rules::built_ins(),
            error: None,
            path,
        },
        Err(error) => RulesPayload {
            next_id: rules::new_id(&rules::Rules::default()),
            rules: Vec::new(),
            built_ins: rules::built_ins(),
            error: Some(error.to_string()),
            path,
        },
    }
}

#[tauri::command]
fn get_rules(app: tauri::AppHandle) -> RulesPayload {
    rules_payload(&settings::config_dir(&app))
}

/// Loads, mutates, validates and saves. A rules.json that will not parse
/// refuses the write rather than being overwritten: the file may be the
/// only copy of rules the user wrote by hand, and a typo in it is not
/// permission to delete them.
fn change_rules(
    config_dir: &std::path::Path,
    change: impl FnOnce(&mut rules::Rules) -> Result<(), rules::RuleError>,
) -> Result<(rules::Rules, Vec<ledger::Input>), String> {
    let mut loaded = rules::load_result(config_dir).map_err(|e| e.to_string())?;
    change(&mut loaded).map_err(|e| e.to_string())?;
    rules::validate(&loaded).map_err(|e| e.to_string())?;
    let before = ledger::hash_file(&rules::rules_path(config_dir))
        .map(|input| vec![input])
        .unwrap_or_default();
    rules::save(config_dir, &loaded).map_err(|e| e.to_string())?;
    Ok((loaded, before))
}

/// One write path for all three verbs: mutate, validate, save, ledger.
fn write_rules(
    app: &tauri::AppHandle,
    action: &str,
    change: impl FnOnce(&mut rules::Rules) -> Result<(), rules::RuleError>,
) -> Result<RulesPayload, String> {
    let config_dir = settings::config_dir(app);
    let (loaded, before) = change_rules(&config_dir, change)?;
    if let Some(folder) = settings::load(app).folder {
        let _ = ledger::append(
            &folder,
            &ledger::Entry {
                at: chrono::Local::now(),
                trigger: ledger::Trigger::Settings,
                action: action.to_string(),
                prompt_id: None,
                prompt_sha256: None,
                engine: None,
                inputs: before,
                output: serde_json::to_string_pretty(&loaded).ok(),
                reasoning: None,
                disposition: ledger::Disposition::Applied,
            },
        );
    }
    Ok(RulesPayload {
        next_id: rules::new_id(&loaded),
        rules: loaded.rules,
        built_ins: rules::built_ins(),
        error: None,
        path: rules::rules_path(&config_dir).to_string_lossy().to_string(),
    })
}

#[tauri::command]
fn add_rule(app: tauri::AppHandle, rule: rules::Rule) -> Result<RulesPayload, String> {
    write_rules(&app, "add_rule", |set| set.add(rule))
}

#[tauri::command]
fn update_rule(app: tauri::AppHandle, rule: rules::Rule) -> Result<RulesPayload, String> {
    write_rules(&app, "update_rule", |set| set.update(rule))
}

#[tauri::command]
fn remove_rule(app: tauri::AppHandle, id: String) -> Result<RulesPayload, String> {
    write_rules(&app, "remove_rule", |set| set.remove(&id))
}

#[derive(serde::Serialize)]
struct PromptPayload {
    id: String,
    text: String,
    customised: bool,
    path: String,
}

fn prompt_payload(app: &tauri::AppHandle, id: prompt::PromptId) -> PromptPayload {
    let config_dir = settings::config_dir(app);
    PromptPayload {
        id: id.as_str().to_string(),
        text: prompt::current(&config_dir, id),
        customised: prompt::is_customised(&config_dir, id),
        path: prompt::prompt_path(&config_dir, id)
            .to_string_lossy()
            .to_string(),
    }
}

#[tauri::command]
fn get_prompt(app: tauri::AppHandle, id: Option<String>) -> Result<PromptPayload, String> {
    Ok(prompt_payload(&app, prompt_id(id)?))
}

#[tauri::command]
fn set_prompt(
    app: tauri::AppHandle,
    id: Option<String>,
    text: String,
) -> Result<PromptPayload, String> {
    let prompt_id = prompt_id(id)?;
    let config_dir = settings::config_dir(&app);
    let before = ledger::hash_file(&prompt::prompt_path(&config_dir, prompt_id))
        .map(|input| vec![input])
        .unwrap_or_default();
    prompt::set(&config_dir, prompt_id, &text).map_err(|e| e.to_string())?;
    if let Some(folder) = settings::load(&app).folder {
        let _ = ledger::append(
            &folder,
            &ledger::Entry {
                at: chrono::Local::now(),
                trigger: ledger::Trigger::Settings,
                action: "set_prompt".to_string(),
                prompt_id: Some(prompt_id.as_str().to_string()),
                prompt_sha256: Some(ledger::sha256_of(text.as_bytes())),
                engine: None,
                inputs: before,
                output: Some(text),
                reasoning: None,
                disposition: ledger::Disposition::Applied,
            },
        );
    }
    Ok(prompt_payload(&app, prompt_id))
}

#[tauri::command]
fn reset_prompt(app: tauri::AppHandle, id: Option<String>) -> Result<PromptPayload, String> {
    let prompt_id = prompt_id(id)?;
    let config_dir = settings::config_dir(&app);
    prompt::reset(&config_dir, prompt_id).map_err(|e| e.to_string())?;
    if let Some(folder) = settings::load(&app).folder {
        let _ = ledger::append(
            &folder,
            &ledger::Entry {
                at: chrono::Local::now(),
                trigger: ledger::Trigger::Settings,
                action: "reset_prompt".to_string(),
                prompt_id: Some(prompt_id.as_str().to_string()),
                prompt_sha256: None,
                engine: None,
                inputs: Vec::new(),
                output: None,
                reasoning: None,
                disposition: ledger::Disposition::Applied,
            },
        );
    }
    Ok(prompt_payload(&app, prompt_id))
}

/// The proposal store keeps a proposal between `propose` and the Apply the
/// user has not clicked yet. It lives in memory only: an unapplied proposal
/// is not worth persisting across a quit.
#[derive(Default)]
struct ProposalStore(std::sync::Mutex<std::collections::HashMap<String, propose::Proposal>>);

#[tauri::command]
fn read_day_blocks(
    app: tauri::AppHandle,
    date: String,
    file: Option<String>,
) -> Vec<days::RawBlock> {
    let Some(folder) = settings::load(&app).folder else {
        return Vec::new();
    };
    let (Ok(date), Ok(file)) = (parse_date(&date), parse_day_file(file)) else {
        return Vec::new();
    };
    days::read_day(&folder, date, file)
        .map(|text| days::parse_blocks(&text))
        .unwrap_or_default()
}

#[tauri::command]
fn website_totals(app: tauri::AppHandle, date: String) -> Vec<days::UrlTotal> {
    let Some(folder) = settings::load(&app).folder else {
        return Vec::new();
    };
    let Ok(date) = parse_date(&date) else {
        return Vec::new();
    };
    days::website_totals(&folder, date)
}

/// Runs on the blocking pool: the agent can take minutes and must not
/// park the webview.
#[tauri::command]
async fn propose(
    app: tauri::AppHandle,
    target: propose::ProposeTarget,
    selection: propose::Selection,
    instruction: String,
) -> Result<propose::Proposal, propose::ProposeError> {
    let loaded = settings::load(&app);
    let agent = loaded.agent.ok_or(propose::ProposeError::NoAgent)?;
    let folder = loaded.folder.ok_or(propose::ProposeError::NoAgent)?;
    // A popover must not park on the agent lock behind a ten-minute
    // summary; say so and let the user try again.
    if agent::is_busy() {
        return Err(propose::ProposeError::AgentFailed {
            stderr: agent::BUSY_MESSAGE.to_string(),
        });
    }
    let config_dir = settings::config_dir(&app);
    let handle = app.clone();
    let proposal = tauri::async_runtime::spawn_blocking(move || {
        propose::propose(
            &config_dir,
            &folder,
            &agent,
            target,
            selection,
            &instruction,
        )
    })
    .await
    .map_err(|e| propose::ProposeError::AgentFailed {
        stderr: e.to_string(),
    })??;
    handle
        .state::<ProposalStore>()
        .0
        .lock()
        .expect("proposal store")
        .insert(proposal.id.clone(), proposal.clone());
    Ok(proposal)
}

#[tauri::command]
fn apply_proposal(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let proposal = app
        .state::<ProposalStore>()
        .0
        .lock()
        .expect("proposal store")
        .remove(&id)
        .ok_or_else(|| "that proposal is no longer available".to_string())?;
    let folder = settings::load(&app)
        .folder
        .ok_or_else(|| "no capture folder is set".to_string())?;
    propose::apply(&settings::config_dir(&app), &folder, &proposal)
}

#[tauri::command]
fn discard_proposal(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let proposal = app
        .state::<ProposalStore>()
        .0
        .lock()
        .expect("proposal store")
        .remove(&id)
        .ok_or_else(|| "that proposal is no longer available".to_string())?;
    let folder = settings::load(&app)
        .folder
        .ok_or_else(|| "no capture folder is set".to_string())?;
    propose::discard(&folder, &proposal).map_err(|e| e.to_string())
}

#[tauri::command]
fn copy_context(selection: propose::Selection) -> String {
    propose::copy_as_context(&selection)
}

#[derive(Serialize)]
struct SummariseNowPayload {
    job_id: String,
}

/// The wire shape of one job. Flat, because the window polls it by id and
/// wants the failure text beside the status rather than nested inside it.
#[derive(Serialize)]
struct JobSummaryPayload {
    id: String,
    date: String,
    status: String,
    stderr: Option<String>,
    step: Option<String>,
}

impl From<jobs::JobSummary> for JobSummaryPayload {
    fn from(job: jobs::JobSummary) -> Self {
        let (status, stderr) = match job.status {
            jobs::JobStatus::Queued => ("queued", None),
            jobs::JobStatus::Running => ("running", None),
            jobs::JobStatus::Done => ("done", None),
            jobs::JobStatus::Failed { stderr } => ("failed", Some(stderr)),
            jobs::JobStatus::Cancelled => ("cancelled", None),
        };
        JobSummaryPayload {
            id: job.id,
            date: job.date.to_string(),
            status: status.to_string(),
            stderr,
            step: job.step,
        }
    }
}

/// Queues the run rather than starting it. The queue is what keeps
/// on-demand and scheduled runs serial: two agent processes writing the
/// same summary is the failure this closes.
#[tauri::command]
fn summarise_now(app: tauri::AppHandle, date: String) -> Result<SummariseNowPayload, String> {
    let parsed = parse_date(&date)?;
    let config = settings::load(&app);
    if config.folder.is_none() {
        return Err("no capture folder is set".to_string());
    }
    if config.agent.is_none() {
        return Err("no agent is connected".to_string());
    }
    let id = app
        .state::<jobs::JobQueue>()
        .enqueue_summarise_with(parsed, ledger::Trigger::OnDemand);
    Ok(SummariseNowPayload {
        job_id: id.to_string(),
    })
}

#[tauri::command]
fn read_kb(app: tauri::AppHandle, date: String, file: Option<String>) -> Option<String> {
    let folder = settings::load(&app).folder?;
    let parsed = parse_date(&date).ok()?;
    crate::ingest::read_kb(&folder, parsed, file.as_deref())
}

#[derive(Serialize)]
struct IngestNowPayload {
    job_id: String,
}

#[tauri::command]
fn ingest_now(
    app: tauri::AppHandle,
    date: String,
    force: bool,
) -> Result<IngestNowPayload, String> {
    let parsed = parse_date(&date)?;
    let config = settings::load(&app);
    if config.folder.is_none() {
        return Err("no capture folder is set".to_string());
    }
    if config.agent.is_none() {
        return Err("no agent is connected".to_string());
    }
    let id =
        app.state::<jobs::JobQueue>()
            .enqueue_ingest_with(parsed, force, ledger::Trigger::OnDemand);
    Ok(IngestNowPayload {
        job_id: id.to_string(),
    })
}

/// Parse all dates before enqueueing any of them. Enqueuing schedules real
/// work, so failing part way through would leave earlier days running with
/// their ids discarded and no way for the caller to poll them.
fn parse_dates(dates: &[String]) -> Result<Vec<chrono::NaiveDate>, String> {
    dates.iter().map(|date| parse_date(date)).collect()
}

/// Enqueues one summarise per date and hands back the job ids in the same
/// order, so the window can poll the batch it just started.
///
/// The caller picks the set. The Overview map already holds the day list it
/// draws, so deciding "has capture, has no summary" there avoids a second
/// implementation of the same rule. A day summarised by something else in
/// between is simply summarised twice.
#[tauri::command]
fn summarise_days(app: tauri::AppHandle, dates: Vec<String>) -> Result<Vec<String>, String> {
    let config = settings::load(&app);
    if config.folder.is_none() {
        return Err("no capture folder is set".to_string());
    }
    if config.agent.is_none() {
        return Err("no agent is connected".to_string());
    }
    let parsed = parse_dates(&dates)?;
    let queue = app.state::<jobs::JobQueue>();
    let ids = parsed
        .into_iter()
        .map(|date| {
            queue
                .enqueue_summarise_with(date, ledger::Trigger::OnDemand)
                .to_string()
        })
        .collect();
    Ok(ids)
}

/// Stops a batch. The day already in flight finishes; everything after it is
/// dropped. Returns how many were still queued, which is informational: the
/// runner may hold more, so the window counts its own cancelled ids.
#[tauri::command]
fn cancel_queued_summaries(app: tauri::AppHandle) -> usize {
    app.state::<jobs::JobQueue>().cancel_queued()
}

/// The ids of the jobs still queued or running. The window calls this on
/// mount and adopts them as its current batch: the runner keeps working
/// after the webview is destroyed, so a window closed mid-run and reopened
/// would otherwise show no batch, leave Stop disabled while days are still
/// being summarised, and offer to enqueue those same days again.
#[tauri::command]
fn running_batch(app: tauri::AppHandle) -> Vec<String> {
    app.state::<jobs::JobQueue>().outstanding()
}

/// One job by id, queued or finished, for the window to poll after it has
/// pressed Summarise.
#[tauri::command]
fn job_state(app: tauri::AppHandle, job_id: String) -> Option<JobSummaryPayload> {
    app.state::<jobs::JobQueue>()
        .find(&job_id)
        .map(JobSummaryPayload::from)
}

/// The last outcome of any run, scheduled or on demand. The tray reads it.
#[tauri::command]
fn job_status(state: tauri::State<jobs::JobState>) -> Option<jobs::Outcome> {
    state.last_outcome()
}

/// The login-shell environment, captured once and reused. Computing it
/// spawns two interactive shells and costs about half a second, which is
/// not a price to pay four times every time the Settings page opens. The
/// cache is warmed on a background thread at startup and can be rebuilt
/// from Settings after installing a CLI.
#[derive(Default)]
pub struct AgentEnv(std::sync::Mutex<Option<std::collections::HashMap<String, String>>>);

impl AgentEnv {
    fn get(&self) -> std::collections::HashMap<String, String> {
        let mut slot = self.0.lock().expect("agent env");
        if slot.is_none() {
            *slot = Some(agent::login_shell_env());
        }
        slot.clone().unwrap_or_default()
    }

    fn refresh(&self) -> std::collections::HashMap<String, String> {
        let fresh = agent::login_shell_env();
        *self.0.lock().expect("agent env") = Some(fresh.clone());
        fresh
    }
}

pub(crate) fn agent_env(app: &tauri::AppHandle) -> std::collections::HashMap<String, String> {
    app.state::<AgentEnv>().get()
}

/// Rebuilds the cached environment. Called from Settings after installing
/// or moving an agent CLI, so a detect can find it without a relaunch.
#[tauri::command]
async fn refresh_agent_env(app: tauri::AppHandle) -> Vec<settings::Agent> {
    tauri::async_runtime::spawn_blocking(move || {
        let env = app.state::<AgentEnv>().refresh();
        agent::detect(&env)
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
fn agent_detect(app: tauri::AppHandle) -> Vec<settings::Agent> {
    agent::detect(&agent_env(&app))
}

/// Proves the connection now, in front of someone who can fix it, rather
/// than at 6am six weeks from now.
#[tauri::command]
async fn agent_test(
    app: tauri::AppHandle,
    agent_config: settings::Agent,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if agent::is_busy() {
            return Err(agent::BUSY_MESSAGE.to_string());
        }
        let mut probe = agent_config;
        // A test must never park the window for ten minutes.
        probe.timeout_secs = probe.timeout_secs.min(60);
        let result =
            agent::run_with_env(&probe, "Reply with exactly the word: ok", &agent_env(&app))
                .map_err(|e| e.to_string());
        // A test spends a model call under the user's subscription, so it
        // belongs in the record like any other agent run.
        if let Some(folder) = settings::load(&app).folder {
            ledger_agent_test(&folder, &probe, &result);
        }
        result
    })
    .await
    .map_err(|e| e.to_string())?
}

/// One entry per agent test: what was run, what came back verbatim, and
/// whether it worked. The action string stays "engine_test": it is written
/// into entries in the user's capture folder, and a query over their ledger
/// should not have to know which app version wrote each line.
fn ledger_agent_test(
    folder: &std::path::Path,
    agent_config: &settings::Agent,
    result: &Result<String, String>,
) {
    let (output, disposition) = match result {
        Ok(text) => (Some(text.clone()), ledger::Disposition::Accepted),
        Err(stderr) => (
            None,
            ledger::Disposition::Failed {
                stderr: stderr.clone(),
            },
        ),
    };
    let entry = ledger::Entry {
        at: chrono::Local::now(),
        trigger: ledger::Trigger::OnDemand,
        action: "engine_test".to_string(),
        prompt_id: None,
        prompt_sha256: None,
        engine: Some(agent_config.label.clone()),
        inputs: Vec::new(),
        output,
        reasoning: None,
        disposition,
    };
    if let Err(error) = ledger::append(folder, &entry) {
        eprintln!("[ledger] could not record the agent test: {error}");
    }
}

/// Whether the agent is signed in, without spending a model call. Ten
/// second cap inside `auth_state`; never called on the schedule.
#[tauri::command]
async fn agent_auth(app: tauri::AppHandle, agent_config: settings::Agent) -> agent::AuthState {
    tauri::async_runtime::spawn_blocking(move || agent::auth_state(&agent_config, &agent_env(&app)))
        .await
        .unwrap_or(agent::AuthState::Unknown)
}

/// Development diagnostics for the audio path (see src/lib/soundDiag.ts).
/// Appends one line to sound-diag.log in the app data dir and echoes it to
/// stderr, so a late cue can be traced without the inspector open.
#[tauri::command]
fn sound_diag(app: tauri::AppHandle, line: String) {
    eprintln!("[sound] {line}");
    if let Ok(dir) = app.path().app_data_dir() {
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("sound-diag.log"))
        {
            use std::io::Write;
            let _ = writeln!(file, "{line}");
        }
    }
}

#[tauri::command]
fn get_settings(app: tauri::AppHandle) -> settings::Settings {
    settings::load(&app)
}

/// Writes settings.json and records the write. The hash is taken before
/// the file changes, because that is what makes the entry reproducible:
/// the ledger names the input the actor saw. The entry is written after
/// the save, so a failed save leaves no entry claiming otherwise.
pub(crate) fn save_settings_recorded(
    config_dir: &std::path::Path,
    folder: Option<&std::path::Path>,
    action: &str,
    next: &settings::Settings,
) -> Result<(), String> {
    let path = config_dir.join("settings.json");
    let inputs = ledger::hash_file(&path)
        .map(|input| vec![input])
        .unwrap_or_default();
    settings::write_to(&path, next).map_err(|e| e.to_string())?;
    let Some(folder) = folder else {
        return Ok(());
    };
    let entry = ledger::Entry {
        at: chrono::Local::now(),
        trigger: ledger::Trigger::Settings,
        action: action.to_string(),
        prompt_id: None,
        prompt_sha256: None,
        engine: None,
        inputs,
        output: serde_json::to_string_pretty(next).ok(),
        reasoning: None,
        disposition: ledger::Disposition::Applied,
    };
    if let Err(error) = ledger::append(folder, &entry) {
        eprintln!("[ledger] could not record {action}: {error}");
    }
    Ok(())
}

/// Saves and then applies, exactly as the MCP `set_config` handler does:
/// a recording knob changed on the Settings page has to reach the running
/// poll thread, not wait for the next launch. Async so the restart's wait
/// for the poll thread happens off the main thread.
#[tauri::command]
async fn set_settings(app: tauri::AppHandle, next: settings::Settings) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let previous = settings::load(&app);
        let folder = next.folder.clone().or_else(|| previous.folder.clone());
        save_settings_recorded(
            &settings::config_dir(&app),
            folder.as_deref(),
            "set_settings",
            &next,
        )?;
        apply_settings_change(&app, &previous, &next);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The setting is the truth and the OS registration follows it. Writing one
/// without the other is how a toggle ends up lying about what the machine
/// will actually do. Both the toggle and the MCP `set_config` handler come
/// through here; `record_as` is None when the caller has already recorded
/// the write, so the change is ledgered once rather than twice.
pub(crate) fn set_launch_at_login_inner(
    app: &tauri::AppHandle,
    enabled: bool,
    record_as: Option<&str>,
) -> Result<(), String> {
    let mut config = settings::load(app);
    config.launch_at_login = enabled;
    match record_as {
        Some(action) => save_settings_recorded(
            &settings::config_dir(app),
            config.folder.clone().as_deref(),
            action,
            &config,
        )?,
        None => settings::save(app, &config).map_err(|e| e.to_string())?,
    }
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())
    } else {
        manager.disable().map_err(|e| e.to_string())
    }
}

/// The Overview tab's way back to first-run setup, for the case it is
/// reporting: permission missing or no folder chosen.
#[tauri::command]
fn open_setup(app: tauri::AppHandle) {
    open_setup_window(&app);
}

#[tauri::command]
fn set_launch_at_login(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    set_launch_at_login_inner(&app, enabled, Some("set_launch_at_login"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn date() -> chrono::NaiveDate {
        chrono::NaiveDate::from_ymd_opt(2026, 8, 28).unwrap()
    }

    #[test]
    fn target_path_names_the_three_day_files_and_the_summary() {
        let f = std::path::Path::new("/f");
        let d = chrono::NaiveDate::from_ymd_opt(2026, 9, 2).unwrap();
        assert_eq!(
            target_path(f, d, "messages").unwrap(),
            std::path::PathBuf::from("/f/Days/2026-09-02/messages.md")
        );
        assert_eq!(
            target_path(f, d, "summary").unwrap(),
            std::path::PathBuf::from("/f/Summaries/2026-09-02.md")
        );
        assert_eq!(
            target_path(f, d, "kb").unwrap(),
            std::path::PathBuf::from("/f/KB/2026-09-02/threads.md")
        );
        assert!(target_path(f, d, "day").is_none());
    }

    #[test]
    fn which_selects_the_day_file_or_its_summary() {
        let folder = Path::new("/tmp/ac");
        assert_eq!(
            target_path(folder, date(), "apps"),
            Some(PathBuf::from("/tmp/ac/Days/2026-08-28/apps.md"))
        );
        assert_eq!(
            target_path(folder, date(), "summary"),
            Some(PathBuf::from("/tmp/ac/Summaries/2026-08-28.md"))
        );
    }

    #[test]
    fn an_unknown_target_opens_nothing_rather_than_guessing() {
        assert_eq!(target_path(Path::new("/tmp/ac"), date(), "ledger"), None);
    }

    fn entries_in(folder: &Path) -> String {
        let path = ledger::ledger_path(folder, chrono::Local::now().date_naive());
        std::fs::read_to_string(path).unwrap_or_default()
    }

    #[test]
    fn a_customised_prompt_opens_the_users_own_file() {
        let config = tempfile::tempdir().unwrap();
        let temp = tempfile::tempdir().unwrap();
        prompt::set(
            config.path(),
            prompt::PromptId::DayContext,
            prompt::PromptId::DayContext.bundled(),
        )
        .unwrap();
        let target =
            prompt_editor_target(config.path(), temp.path(), prompt::PromptId::DayContext).unwrap();
        assert_eq!(
            target,
            prompt::prompt_path(config.path(), prompt::PromptId::DayContext)
        );
    }

    #[test]
    fn the_bundled_prompt_opens_as_a_read_only_copy_and_stays_uncustomised() {
        let config = tempfile::tempdir().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let target =
            prompt_editor_target(config.path(), temp.path(), prompt::PromptId::DayContext).unwrap();

        assert!(target.starts_with(temp.path()));
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            prompt::PromptId::DayContext.bundled()
        );
        let mode = std::fs::metadata(&target).unwrap().permissions().mode();
        assert_eq!(mode & 0o222, 0, "the copy is read only");
        assert!(
            !prompt::is_customised(config.path(), prompt::PromptId::DayContext),
            "opening the prompt is not customising it"
        );

        // Opening it twice must not fail on the read-only copy it left.
        assert!(
            prompt_editor_target(config.path(), temp.path(), prompt::PromptId::DayContext,).is_ok()
        );
    }

    #[test]
    fn the_pending_day_is_handed_over_once_and_then_forgotten() {
        let pending = PendingOpenDay::default();
        assert_eq!(pending.take(), None);
        pending.put("2026-08-24".to_string());
        // A second open_day before the window asks replaces the first: the
        // most recent request is the one that matters.
        pending.put("2026-08-25".to_string());
        assert_eq!(pending.take().as_deref(), Some("2026-08-25"));
        assert_eq!(pending.take(), None);
    }

    #[test]
    fn a_rules_file_that_will_not_parse_is_reported_rather_than_read_as_no_rules() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(rules::rules_path(dir.path()), "{ not json").unwrap();

        let payload = rules_payload(dir.path());
        assert!(payload.rules.is_empty());
        let error = payload.error.expect("the parse failure");
        assert!(error.contains("not a rules file"), "{error}");
        assert!(!payload.built_ins.is_empty(), "the built-ins still show");

        // And a write refuses with the same sentence rather than replacing
        // the file the user may have hand-edited.
        let rule = rules::Rule {
            id: "r1".to_string(),
            target: rules::Target::App("Linear".to_string()),
            action: rules::Action::Exclude,
            note: None,
        };
        let refusal = change_rules(dir.path(), |set| set.add(rule)).unwrap_err();
        assert_eq!(refusal, error);
        assert_eq!(
            std::fs::read_to_string(rules::rules_path(dir.path())).unwrap(),
            "{ not json"
        );
    }

    #[test]
    fn a_write_against_a_readable_rules_file_still_goes_through() {
        let dir = tempfile::tempdir().unwrap();
        let rule = rules::Rule {
            id: "r1".to_string(),
            target: rules::Target::App("Linear".to_string()),
            action: rules::Action::Exclude,
            note: None,
        };
        let (saved, _) = change_rules(dir.path(), |set| set.add(rule)).unwrap();
        assert_eq!(saved.rules.len(), 1);
        assert!(rules_payload(dir.path()).error.is_none());
    }

    #[test]
    fn a_settings_write_is_recorded_once_with_the_previous_file_as_its_input() {
        let config = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        let previous = settings::Settings::default();
        settings::write_to(&config.path().join("settings.json"), &previous).unwrap();
        let before = ledger::hash_file(&config.path().join("settings.json")).unwrap();

        let next = settings::Settings {
            interval_secs: 30,
            ..previous
        };
        save_settings_recorded(config.path(), Some(folder.path()), "set_settings", &next).unwrap();

        assert_eq!(
            settings::read_from(&config.path().join("settings.json")).interval_secs,
            30
        );
        let text = entries_in(folder.path());
        assert_eq!(text.matches("\n## ").count(), 1, "exactly one entry");
        assert!(text.contains("set_settings"));
        assert!(text.contains("- trigger: settings"));
        assert!(text.contains("- disposition: applied"));
        assert!(
            text.contains(&before.sha256),
            "the input is the file as it was before the write"
        );
        assert!(text.contains("\"interval_secs\": 30"));
    }

    #[test]
    fn the_launch_at_login_write_is_recorded_under_its_own_action() {
        let config = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        let next = settings::Settings {
            launch_at_login: false,
            ..settings::Settings::default()
        };
        save_settings_recorded(
            config.path(),
            Some(folder.path()),
            "set_launch_at_login",
            &next,
        )
        .unwrap();

        let text = entries_in(folder.path());
        assert_eq!(text.matches("\n## ").count(), 1);
        assert!(text.contains("set_launch_at_login"));
        // No file existed to hash, so the entry names no input rather than
        // inventing one.
        assert!(!text.contains("- input: "));
    }

    #[test]
    fn an_engine_test_is_recorded_with_its_output_and_its_failures() {
        let folder = tempfile::tempdir().unwrap();
        let agent_config = settings::Agent {
            label: "Claude Code".to_string(),
            command: "/bin/echo".to_string(),
            args: Vec::new(),
            timeout_secs: 60,
        };
        ledger_agent_test(folder.path(), &agent_config, &Ok("ok".to_string()));
        let text = entries_in(folder.path());
        assert_eq!(text.matches("\n## ").count(), 1);
        assert!(text.contains("engine_test"));
        assert!(text.contains("- trigger: on demand"));
        assert!(text.contains("- engine: Claude Code"));
        assert!(text.contains("- disposition: accepted"));
        assert!(text.contains("ok"));

        ledger_agent_test(
            folder.path(),
            &agent_config,
            &Err("not logged in".to_string()),
        );
        let text = entries_in(folder.path());
        assert_eq!(text.matches("\n## ").count(), 2);
        assert!(text.contains("failed: not logged in"));
    }

    #[test]
    fn a_changed_recording_knob_asks_for_a_restart() {
        let previous = settings::Settings::default();
        let next = settings::Settings {
            interval_secs: previous.interval_secs + 5,
            ..previous.clone()
        };
        assert!(restart_needed(&previous, &next));
        let dwell = settings::Settings {
            min_dwell_secs: previous.min_dwell_secs + 5,
            ..previous.clone()
        };
        assert!(restart_needed(&previous, &dwell));
    }

    #[test]
    fn a_knob_the_poll_loop_re_reads_does_not_restart_capture() {
        let previous = settings::Settings::default();
        let next = settings::Settings {
            max_block_chars: 2000,
            write_references: false,
            ..previous.clone()
        };
        assert!(!restart_needed(&previous, &next));
        assert!(!restart_needed(&previous, &previous.clone()));
    }

    #[test]
    fn a_bad_date_is_an_error_rather_than_a_panic() {
        assert!(parse_date("not-a-date").is_err());
        assert!(parse_date("2026-08-28").is_ok());
    }

    #[test]
    fn parse_dates_validates_all_before_accepting_any() {
        // Valid dates should parse successfully.
        let valid = vec!["2025-01-15".to_string(), "2025-02-20".to_string()];
        let result = parse_dates(&valid);
        assert!(result.is_ok());
        let dates = result.unwrap();
        assert_eq!(dates.len(), 2);

        // If any date is invalid, the whole batch fails, not just the bad one.
        let mixed = vec![
            "2025-01-15".to_string(),
            "invalid-date".to_string(),
            "2025-02-20".to_string(),
        ];
        let result = parse_dates(&mixed);
        assert!(result.is_err());
    }

    #[test]
    fn parse_dates_preserves_order() {
        use chrono::Datelike;

        let dates = vec![
            "2025-03-10".to_string(),
            "2025-01-05".to_string(),
            "2025-02-15".to_string(),
        ];
        let result = parse_dates(&dates).expect("dates should parse");
        assert_eq!(result[0].month(), 3);
        assert_eq!(result[1].month(), 1);
        assert_eq!(result[2].month(), 2);
    }
}

#[cfg(test)]
mod registration_tests {
    #[test]
    fn the_registration_command_string_quotes_a_path_with_spaces() {
        // The bundle path contains a space, always: "Ambient Context.app".
        let quoted =
            super::shell_quote("/Applications/Ambient Context.app/Contents/MacOS/ambient-context");
        assert_eq!(
            quoted,
            "\"/Applications/Ambient Context.app/Contents/MacOS/ambient-context\""
        );
    }

    #[test]
    fn a_path_without_spaces_is_left_bare() {
        assert_eq!(
            super::shell_quote("/usr/local/bin/ambient-context"),
            "/usr/local/bin/ambient-context"
        );
    }
}

#[tauri::command]
fn list_days(app: tauri::AppHandle) -> Vec<days::DayEntry> {
    match settings::load(&app).folder {
        Some(folder) => days::list_days(&folder),
        None => Vec::new(),
    }
}

#[tauri::command]
fn days_in_month(app: tauri::AppHandle, year: i32, month: u32) -> Vec<days::DayEntry> {
    match settings::load(&app).folder {
        Some(folder) => days::days_in_month(&folder, year, month),
        None => Vec::new(),
    }
}

#[tauri::command]
fn read_day(app: tauri::AppHandle, date: String, file: Option<String>) -> Option<String> {
    let folder = settings::load(&app).folder?;
    let file = parse_day_file(file).ok()?;
    days::read_day(&folder, parse_date(&date).ok()?, file)
}

#[tauri::command]
fn read_summary(app: tauri::AppHandle, date: String) -> Option<String> {
    let folder = settings::load(&app).folder?;
    days::read_summary(&folder, parse_date(&date).ok()?)
}

/// The browsing window, unlike the setup card, is resizable and keeps the
/// native titlebar: it is a document window and should behave like one.
pub fn open_main_window(app: &tauri::AppHandle) {
    open_main_window_with_tab(app, None);
}

/// Opens the browsing window on a given tab. An already-open window is
/// switched with an event; a cold open carries the tab in the URL so React
/// Strict Mode cannot consume the pending state on remount.
pub fn open_main_window_on_tab(app: &tauri::AppHandle, tab: &str) {
    open_main_window_with_tab(app, Some(tab));
}

fn main_window_url(tab: Option<&str>) -> WebviewUrl {
    let path = match tab {
        Some(tab) => format!("index.html?view=main&tab={tab}"),
        None => String::from("index.html?view=main"),
    };
    WebviewUrl::App(path.into())
}

fn open_main_window_with_tab(app: &tauri::AppHandle, tab: Option<&str>) {
    sync_activation_policy(app, true);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        window_chrome::centre_traffic_lights(&window);
        let _ = window.set_focus();
        if let Some(tab) = tab {
            let _ = window.emit("open-tab", tab);
        }
        return;
    }
    // The traffic lights and native resizing stay; the title bar itself is
    // transparent and the page draws its own under them, which is why the
    // drawn bar carries a left inset wide enough to clear the buttons.
    if let Ok(window) = WebviewWindowBuilder::new(app, "main", main_window_url(tab))
        .title("Ambient Context")
        .inner_size(1000.0, 700.0)
        .min_inner_size(820.0, 560.0)
        .resizable(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .build()
    {
        let _ = window.show();
        window_chrome::centre_traffic_lights(&window);
        let _ = window.set_focus();
    }
}

/// The About window: a small fixed dialog with no native chrome, the same
/// shape as setup. Opened from the menu bar icon's dropdown.
pub fn open_about_window(app: &tauri::AppHandle) {
    sync_activation_policy(app, true);
    if let Some(window) = app.get_webview_window("about") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    // Built hidden and shown from the page load callback. A window is
    // visible the moment it is built, and an unpainted webview is white, so
    // opening this window flashed a white rectangle before the dialog
    // appeared. Showing it here rather than from the frontend keeps the
    // About window's capability as narrow as it is: it has no
    // core:window:allow-show, and does not need one.
    if let Ok(window) = WebviewWindowBuilder::new(
        app,
        "about",
        WebviewUrl::App("index.html?view=about".into()),
    )
    .title("About Ambient Context")
    .inner_size(380.0, 400.0)
    .resizable(false)
    .decorations(false)
    .visible(false)
    .on_page_load(|window, payload| {
        if payload.event() == tauri::webview::PageLoadEvent::Finished {
            let _ = window.show();
            let _ = window.set_focus();
        }
    })
    .build()
    {
        let _ = window;
    }
}

/// The version the About window shows, read from the crate rather than
/// duplicated in the page, so a release bump cannot leave it stale.
#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// An Accessory app has no Dock icon and cannot properly take focus, which
/// is right for a menu bar app and wrong for a window you read in. Raise to
/// Regular while any window is open, and drop back when the last one goes.
fn sync_activation_policy(app: &tauri::AppHandle, opening: bool) {
    #[cfg(target_os = "macos")]
    {
        let any_open = opening
            || app
                .webview_windows()
                .values()
                .any(|w| w.is_visible().unwrap_or(false));
        let policy = if any_open {
            tauri::ActivationPolicy::Regular
        } else {
            tauri::ActivationPolicy::Accessory
        };
        let _ = app.set_activation_policy(policy);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, opening);
}

/// The day an `open_day` call asked for, held until the window asks for
/// it. An event alone loses the race on a cold open: the webview is built,
/// the event goes out, and the page starts listening a moment later.
#[derive(Default)]
pub struct PendingOpenDay(std::sync::Mutex<Option<String>>);

impl PendingOpenDay {
    fn put(&self, date: String) {
        *self.0.lock().expect("pending open day") = Some(date);
    }

    fn take(&self) -> Option<String> {
        self.0.lock().expect("pending open day").take()
    }
}

/// Read once, on mount, by the Day view. Taking it clears it, so a later
/// reload shows the day the user last chose rather than replaying an old
/// request.
#[tauri::command]
fn take_pending_day(app: tauri::AppHandle) -> Option<String> {
    app.state::<PendingOpenDay>().take()
}

/// Opens the browsing window on a given day, for MCP `open_day` and any
/// later handoff that ends "check it looks right". The date is left where
/// the page can collect it when it mounts, and emitted as well for the
/// window that is already open and already listening.
pub fn open_main_window_on(app: &tauri::AppHandle, date: chrono::NaiveDate) {
    let date = date.to_string();
    app.state::<PendingOpenDay>().put(date.clone());
    open_main_window(app);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit("open-day", date);
    }
}

/// The three knobs the poll thread reads once, at the top of its loop, and
/// therefore the only ones a restart is needed for. Folder, block size,
/// references and the extra redaction patterns are all re-read on every
/// poll, so changing them takes effect without stopping anything.
fn restart_needed(previous: &settings::Settings, next: &settings::Settings) -> bool {
    previous.interval_secs != next.interval_secs
        || previous.min_dwell_secs != next.min_dwell_secs
        || previous.similarity_threshold != next.similarity_threshold
}

/// Applies a settings change written by any surface: capture restarts with
/// the new recording knobs if it is running, and nothing else needs a
/// restart because every other consumer re-reads settings as it goes.
/// Both the Settings page and the MCP `set_config` handler come through
/// here, so the two surfaces cannot drift apart.
pub fn apply_settings_change(
    app: &tauri::AppHandle,
    previous: &settings::Settings,
    next: &settings::Settings,
) {
    if !restart_needed(previous, next) {
        return;
    }
    let state = app.state::<capture::CaptureState>().inner().clone();
    if state.is_running() && next.folder.is_some() {
        // Only start once the old thread has actually left. Two poll
        // threads with their own segmenters append to the same day file,
        // so a stop that has not finished is a reason to leave capture off
        // rather than to start another.
        if capture::stop(&state) {
            capture::start(app.clone(), &state, next.clone());
        } else {
            eprintln!("[capture] the poll thread did not stop; not restarting it");
        }
    }
}

/// Wraps a path in double quotes only when it needs them. The bundle path
/// always contains a space, so an unquoted claude mcp add line is wrong on
/// every real install.
pub(crate) fn shell_quote(path: &str) -> String {
    if path.contains(' ') {
        format!("\"{path}\"")
    } else {
        path.to_string()
    }
}

#[derive(Serialize)]
struct McpRegistration {
    binary: String,
    quoted_binary: String,
    running: bool,
    last_write: Option<serde_json::Value>,
}

#[tauri::command]
fn mcp_registration(app: tauri::AppHandle) -> McpRegistration {
    let binary = std::env::current_exe()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();
    let data_dir = app.path().app_data_dir().unwrap_or_default();
    let last_write = std::fs::read_to_string(data_dir.join("last-mcp-write.json"))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok());
    McpRegistration {
        quoted_binary: shell_quote(&binary),
        binary,
        running: ipc::socket_path(&data_dir).exists(),
        last_write,
    }
}

pub fn open_setup_window(app: &tauri::AppHandle) {
    sync_activation_policy(app, true);
    if let Some(window) = app.get_webview_window("setup") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    // An Accessory-policy app is not frontmost when a tray menu item fires,
    // so a freshly built window appears behind whatever has focus and looks
    // like it needs a second click. Focus it explicitly.
    // No native chrome: the page draws its own titlebar, which drags the
    // window and carries the close button.
    if let Ok(window) =
        WebviewWindowBuilder::new(app, "setup", WebviewUrl::App("index.html".into()))
            .title("Settings")
            .inner_size(520.0, 620.0)
            .resizable(false)
            .decorations(false)
            .build()
    {
        let _ = window.set_focus();
    }
}

pub fn check_for_updates(app: &tauri::AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let updater = match handle.updater() {
            Ok(updater) => updater,
            Err(error) => {
                show_update_message(
                    &handle,
                    MessageDialogKind::Error,
                    &format!("Could not check for updates.\n{error}"),
                );
                return;
            }
        };

        match updater.check().await {
            Ok(Some(update)) => {
                let install = handle
                    .dialog()
                    .message(format!(
                        "Version {} is available. Install it now?",
                        update.version
                    ))
                    .title("Ambient Context")
                    .kind(MessageDialogKind::Info)
                    .buttons(MessageDialogButtons::OkCancelCustom(
                        "Install".to_string(),
                        "Later".to_string(),
                    ))
                    .blocking_show();
                if !install {
                    return;
                }
                if let Err(error) = update.download_and_install(|_, _| {}, || {}).await {
                    show_update_message(
                        &handle,
                        MessageDialogKind::Error,
                        &format!("The update failed.\n{error}"),
                    );
                    return;
                }
                handle.restart();
            }
            Ok(None) => {
                show_update_message(
                    &handle,
                    MessageDialogKind::Info,
                    "You're on the latest version.",
                );
            }
            Err(error) => {
                show_update_message(
                    &handle,
                    MessageDialogKind::Error,
                    &format!("Could not check for updates.\n{error}"),
                );
            }
        }
    });
}

fn show_update_message(app: &tauri::AppHandle, kind: MessageDialogKind, message: &str) {
    app.dialog()
        .message(message)
        .title("Ambient Context")
        .kind(kind)
        .blocking_show();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            sound_diag,
            permission_status,
            request_permission,
            current_folder,
            choose_folder,
            use_default_folder,
            capture_status,
            start_if_enabled,
            toggle_capture,
            census_snapshot,
            open_link,
            summarise_now,
            ingest_now,
            summarise_days,
            cancel_queued_summaries,
            running_batch,
            job_status,
            job_state,
            agent_detect,
            refresh_agent_env,
            agent_test,
            agent_auth,
            get_settings,
            set_settings,
            set_launch_at_login,
            open_setup,
            app_version,
            list_days,
            days_in_month,
            read_day,
            read_kb,
            read_summary,
            open_in_editor,
            open_prompt_in_editor,
            reveal_day,
            get_rules,
            add_rule,
            update_rule,
            remove_rule,
            get_prompt,
            set_prompt,
            reset_prompt,
            read_day_blocks,
            website_totals,
            propose,
            apply_proposal,
            discard_proposal,
            copy_context,
            mcp_registration,
            take_pending_day
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            app.manage(capture::CaptureState::new());
            app.manage(AgentEnv::default());
            {
                // Warm the login-shell environment off the launch path: it
                // costs about half a second and nothing needs it yet.
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    let _ = agent_env(&handle);
                });
            }
            {
                // The schedule's own memory: without it every launch looks
                // overdue and fires the backfill a minute after startup.
                let last_run = app
                    .path()
                    .app_data_dir()
                    .ok()
                    .and_then(|dir| jobs::read_last_run(&dir));
                app.manage(jobs::JobState::with_last_run(last_run));
            }
            app.manage(jobs::JobQueue::default());
            app.manage(ProposalStore::default());
            app.manage(PendingOpenDay::default());
            jobs::start(app.handle().clone());
            // The control socket, for the mcp subcommand. A failure to bind is
            // reported to stderr and nothing else: an app that will not start
            // because an MCP socket is busy is worse than an app with no MCP.
            {
                let handle = app.handle().clone();
                let data_dir = handle.path().app_data_dir().expect("app data dir");
                match ipc::bind(&ipc::socket_path(&data_dir)) {
                    Ok(listener) => {
                        std::thread::spawn(move || {
                            ipc::serve(listener, move |request| control::handle(&handle, request));
                        });
                    }
                    Err(error) => eprintln!("control socket unavailable: {error}"),
                }
            }
            tray::build(app.handle())?;

            let config = settings::load(app.handle());
            {
                // A login item removed in System Settings must not leave the
                // toggle claiming otherwise: reconcile once at startup.
                let manager = app.autolaunch();
                let registered = manager.is_enabled().unwrap_or(false);
                if config.launch_at_login && !registered {
                    let _ = manager.enable();
                } else if !config.launch_at_login && registered {
                    let _ = manager.disable();
                }
            }
            if reader::macos::permission_status() == reader::Permission::NotGranted
                || config.folder.is_none()
            {
                open_setup_window(app.handle());
            } else if config.enabled {
                // Recording is the default state of a set-up app. Only an
                // explicit stop (persisted as enabled=false) prevents this.
                let state = app.state::<capture::CaptureState>().inner().clone();
                capture::start(app.handle().clone(), &state, config);
                tray::refresh(app.handle(), true);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                let app = window.app_handle().clone();
                // Destroyed fires before the window leaves the collection on
                // some runs; a short hop to the main thread lets it settle.
                let _ = app.clone().run_on_main_thread(move || {
                    sync_activation_policy(&app, false);
                });
            }
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_app, event| {
            // Closing the setup window must not quit a menu bar app. A
            // window-close exit request carries no code; the Quit menu item
            // calls app.exit(0), which does, and passes through.
            if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
