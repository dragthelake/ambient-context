mod capture;
mod control;
mod days;
mod engine;
mod ipc;
mod jobs;
mod ledger;
pub mod mcp;
mod prompt;
mod propose;
mod prune;
mod reader;
mod redact;
mod rules;
mod segment;
mod settings;
mod summarise;
mod tray;
mod writer;

use serde::Serialize;
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
    let _ = settings::save(&app, &config);
    Some(path.to_string_lossy().to_string())
}

/// The default is ~/Ambient Context, deliberately outside ~/Documents:
/// iCloud's Desktop & Documents sync uploads everything in Documents, and
/// this file must not silently leave the machine.
#[tauri::command]
fn use_default_folder(app: tauri::AppHandle) -> Option<String> {
    let home = app.path().home_dir().ok()?;
    let path = home.join("Ambient Context");
    // Create it now, not at first write: Reveal Folder and Open Today's File
    // are dead until the folder exists, which reads as a broken app.
    std::fs::create_dir_all(&path).ok()?;
    let mut config = settings::load(&app);
    config.folder = Some(path.clone());
    let _ = settings::save(&app, &config);
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

/// Which file a Day view action means. Unknown values return None rather
/// than defaulting, so a typo opens nothing instead of the wrong file.
fn target_path(
    folder: &std::path::Path,
    date: chrono::NaiveDate,
    which: &str,
) -> Option<std::path::PathBuf> {
    match which {
        "day" => Some(writer::file_path(folder, date)),
        "summary" => Some(summarise::summary_path(folder, date)),
        _ => None,
    }
}

#[tauri::command]
fn open_in_editor(app: tauri::AppHandle, date: String, which: String) -> Result<(), String> {
    let config = settings::load(&app);
    let folder = config.folder.clone().ok_or("no capture folder is set")?;
    let parsed = parse_date(&date)?;
    let path = target_path(&folder, parsed, &which).ok_or("there is no such file to open")?;
    if !path.is_file() {
        return Err(format!("there is no {which} file for {date} yet"));
    }
    let mut command = std::process::Command::new("open");
    // No editor configured means the system handler for markdown, which is
    // whatever the user already double-clicks these files with.
    if let Some(editor) = config.editor {
        command.arg("-a").arg(editor);
    }
    command.arg(path).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn reveal_day(app: tauri::AppHandle, date: String) -> Result<(), String> {
    let folder = settings::load(&app)
        .folder
        .ok_or("no capture folder is set")?;
    let parsed = parse_date(&date)?;
    let path = writer::file_path(&folder, parsed);
    // -R selects the file in Finder. A day with no file yet still has a
    // folder worth opening.
    let target = if path.is_file() { path } else { folder };
    std::process::Command::new("open")
        .arg("-R")
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
}

#[tauri::command]
fn get_rules(app: tauri::AppHandle) -> RulesPayload {
    let loaded = rules::load(&settings::config_dir(&app));
    RulesPayload {
        next_id: rules::new_id(&loaded),
        rules: loaded.rules,
        built_ins: rules::built_ins(),
    }
}

/// One write path for all three verbs: mutate, validate, save, ledger.
fn write_rules(
    app: &tauri::AppHandle,
    action: &str,
    change: impl FnOnce(&mut rules::Rules) -> Result<(), rules::RuleError>,
) -> Result<RulesPayload, String> {
    let config_dir = settings::config_dir(app);
    let mut loaded = rules::load(&config_dir);
    change(&mut loaded).map_err(|e| e.to_string())?;
    rules::validate(&loaded).map_err(|e| e.to_string())?;
    let before = ledger::hash_file(&rules::rules_path(&config_dir))
        .map(|input| vec![input])
        .unwrap_or_default();
    rules::save(&config_dir, &loaded).map_err(|e| e.to_string())?;
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
    text: String,
    customised: bool,
    path: String,
}

fn prompt_payload(app: &tauri::AppHandle) -> PromptPayload {
    let config_dir = settings::config_dir(app);
    PromptPayload {
        text: prompt::current(&config_dir),
        customised: prompt::is_customised(&config_dir),
        path: prompt::prompt_path(&config_dir)
            .to_string_lossy()
            .to_string(),
    }
}

#[tauri::command]
fn get_prompt(app: tauri::AppHandle) -> PromptPayload {
    prompt_payload(&app)
}

#[tauri::command]
fn set_prompt(app: tauri::AppHandle, text: String) -> Result<PromptPayload, String> {
    let config_dir = settings::config_dir(&app);
    let before = ledger::hash_file(&prompt::prompt_path(&config_dir))
        .map(|input| vec![input])
        .unwrap_or_default();
    prompt::set(&config_dir, &text).map_err(|e| e.to_string())?;
    if let Some(folder) = settings::load(&app).folder {
        let _ = ledger::append(
            &folder,
            &ledger::Entry {
                at: chrono::Local::now(),
                trigger: ledger::Trigger::Settings,
                action: "set_prompt".to_string(),
                prompt_id: None,
                prompt_sha256: Some(ledger::sha256_of(text.as_bytes())),
                engine: None,
                inputs: before,
                output: Some(text),
                reasoning: None,
                disposition: ledger::Disposition::Applied,
            },
        );
    }
    Ok(prompt_payload(&app))
}

#[tauri::command]
fn reset_prompt(app: tauri::AppHandle) -> Result<PromptPayload, String> {
    let config_dir = settings::config_dir(&app);
    prompt::reset(&config_dir).map_err(|e| e.to_string())?;
    if let Some(folder) = settings::load(&app).folder {
        let _ = ledger::append(
            &folder,
            &ledger::Entry {
                at: chrono::Local::now(),
                trigger: ledger::Trigger::Settings,
                action: "reset_prompt".to_string(),
                prompt_id: None,
                prompt_sha256: None,
                engine: None,
                inputs: Vec::new(),
                output: None,
                reasoning: None,
                disposition: ledger::Disposition::Applied,
            },
        );
    }
    Ok(prompt_payload(&app))
}

/// The proposal store keeps a proposal between `propose` and the Apply the
/// user has not clicked yet. It lives in memory only: an unapplied proposal
/// is not worth persisting across a quit.
#[derive(Default)]
struct ProposalStore(std::sync::Mutex<std::collections::HashMap<String, propose::Proposal>>);

#[tauri::command]
fn read_day_blocks(app: tauri::AppHandle, date: String) -> Vec<days::RawBlock> {
    let Some(folder) = settings::load(&app).folder else {
        return Vec::new();
    };
    let Ok(date) = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d") else {
        return Vec::new();
    };
    days::read_day(&folder, date)
        .map(|text| days::parse_blocks(&text))
        .unwrap_or_default()
}

/// Runs on the blocking pool: the engine can take minutes and must not
/// park the webview.
#[tauri::command]
async fn propose(
    app: tauri::AppHandle,
    target: propose::ProposeTarget,
    selection: propose::Selection,
    instruction: String,
) -> Result<propose::Proposal, propose::ProposeError> {
    let loaded = settings::load(&app);
    let engine = loaded.engine.ok_or(propose::ProposeError::NoEngine)?;
    let folder = loaded.folder.ok_or(propose::ProposeError::NoEngine)?;
    let config_dir = settings::config_dir(&app);
    let handle = app.clone();
    let proposal = tauri::async_runtime::spawn_blocking(move || {
        propose::propose(
            &config_dir,
            &folder,
            &engine,
            target,
            selection,
            &instruction,
        )
    })
    .await
    .map_err(|e| propose::ProposeError::EngineFailed {
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
}

impl From<jobs::JobSummary> for JobSummaryPayload {
    fn from(job: jobs::JobSummary) -> Self {
        let (status, stderr) = match job.status {
            jobs::JobStatus::Queued => ("queued", None),
            jobs::JobStatus::Running => ("running", None),
            jobs::JobStatus::Done => ("done", None),
            jobs::JobStatus::Failed { stderr } => ("failed", Some(stderr)),
        };
        JobSummaryPayload {
            id: job.id,
            date: job.date.to_string(),
            status: status.to_string(),
            stderr,
        }
    }
}

/// Queues the run rather than starting it. The queue is what keeps
/// on-demand and scheduled runs serial: two engine processes writing the
/// same summary is the failure this closes.
#[tauri::command]
fn summarise_now(app: tauri::AppHandle, date: String) -> Result<SummariseNowPayload, String> {
    let parsed = parse_date(&date)?;
    let config = settings::load(&app);
    if config.folder.is_none() {
        return Err("no capture folder is set".to_string());
    }
    if config.engine.is_none() {
        return Err("no engine is connected".to_string());
    }
    let id = app
        .state::<jobs::JobQueue>()
        .enqueue_summarise_with(parsed, ledger::Trigger::OnDemand);
    Ok(SummariseNowPayload {
        job_id: id.to_string(),
    })
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
pub struct EngineEnv(std::sync::Mutex<Option<std::collections::HashMap<String, String>>>);

impl EngineEnv {
    fn get(&self) -> std::collections::HashMap<String, String> {
        let mut slot = self.0.lock().expect("engine env");
        if slot.is_none() {
            *slot = Some(engine::login_shell_env());
        }
        slot.clone().unwrap_or_default()
    }

    fn refresh(&self) -> std::collections::HashMap<String, String> {
        let fresh = engine::login_shell_env();
        *self.0.lock().expect("engine env") = Some(fresh.clone());
        fresh
    }
}

pub(crate) fn engine_env(app: &tauri::AppHandle) -> std::collections::HashMap<String, String> {
    app.state::<EngineEnv>().get()
}

/// Rebuilds the cached environment. Called from Settings after installing
/// or moving an engine CLI, so a detect can find it without a relaunch.
#[tauri::command]
async fn refresh_engine_env(app: tauri::AppHandle) -> Vec<settings::Engine> {
    tauri::async_runtime::spawn_blocking(move || {
        let env = app.state::<EngineEnv>().refresh();
        engine::detect(&env)
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
fn engine_detect(app: tauri::AppHandle) -> Vec<settings::Engine> {
    engine::detect(&engine_env(&app))
}

/// Proves the connection now, in front of someone who can fix it, rather
/// than at 6am six weeks from now.
#[tauri::command]
async fn engine_test(
    app: tauri::AppHandle,
    engine_config: settings::Engine,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut probe = engine_config;
        // A test must never park the window for ten minutes.
        probe.timeout_secs = probe.timeout_secs.min(60);
        let result =
            engine::run_with_env(&probe, "Reply with exactly the word: ok", &engine_env(&app))
                .map_err(|e| e.to_string());
        // A test spends a model call under the user's subscription, so it
        // belongs in the record like any other engine run.
        if let Some(folder) = settings::load(&app).folder {
            ledger_engine_test(&folder, &probe, &result);
        }
        result
    })
    .await
    .map_err(|e| e.to_string())?
}

/// One entry per engine test: what was run, what came back verbatim, and
/// whether it worked.
fn ledger_engine_test(
    folder: &std::path::Path,
    engine_config: &settings::Engine,
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
        engine: Some(engine_config.label.clone()),
        inputs: Vec::new(),
        output,
        reasoning: None,
        disposition,
    };
    if let Err(error) = ledger::append(folder, &entry) {
        eprintln!("[ledger] could not record the engine test: {error}");
    }
}

/// Whether the engine is signed in, without spending a model call. Ten
/// second cap inside `auth_state`; never called on the schedule.
#[tauri::command]
async fn engine_auth(app: tauri::AppHandle, engine_config: settings::Engine) -> engine::AuthState {
    tauri::async_runtime::spawn_blocking(move || {
        engine::auth_state(&engine_config, &engine_env(&app))
    })
    .await
    .unwrap_or(engine::AuthState::Unknown)
}

#[tauri::command]
fn get_settings(app: tauri::AppHandle) -> settings::Settings {
    settings::load(&app)
}

/// Writes settings.json and records the write. The hash is taken before
/// the file changes, because that is what makes the entry reproducible:
/// the ledger names the input the actor saw. The entry is written after
/// the save, so a failed save leaves no entry claiming otherwise.
fn save_settings_recorded(
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
    fn which_selects_the_day_file_or_its_summary() {
        let folder = Path::new("/tmp/ac");
        assert_eq!(
            target_path(folder, date(), "day"),
            Some(PathBuf::from("/tmp/ac/2026-08-28.md"))
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
        let engine_config = settings::Engine {
            label: "Claude Code".to_string(),
            command: "/bin/echo".to_string(),
            args: Vec::new(),
            timeout_secs: 60,
        };
        ledger_engine_test(folder.path(), &engine_config, &Ok("ok".to_string()));
        let text = entries_in(folder.path());
        assert_eq!(text.matches("\n## ").count(), 1);
        assert!(text.contains("engine_test"));
        assert!(text.contains("- trigger: on demand"));
        assert!(text.contains("- engine: Claude Code"));
        assert!(text.contains("- disposition: accepted"));
        assert!(text.contains("ok"));

        ledger_engine_test(
            folder.path(),
            &engine_config,
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
fn read_day(app: tauri::AppHandle, date: String) -> Option<String> {
    let folder = settings::load(&app).folder?;
    days::read_day(&folder, parse_date(&date).ok()?)
}

#[tauri::command]
fn read_summary(app: tauri::AppHandle, date: String) -> Option<String> {
    let folder = settings::load(&app).folder?;
    days::read_summary(&folder, parse_date(&date).ok()?)
}

/// The browsing window, unlike the setup card, is resizable and keeps the
/// native titlebar: it is a document window and should behave like one.
pub fn open_main_window(app: &tauri::AppHandle) {
    sync_activation_policy(app, true);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    if let Ok(window) =
        WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html?view=main".into()))
            .title("Ambient Context")
            .inner_size(1000.0, 700.0)
            .min_inner_size(820.0, 560.0)
            .resizable(true)
            .build()
    {
        let _ = window.set_focus();
    }
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

/// Opens the browsing window on a given day, for MCP `open_day` and any
/// later handoff that ends "check it looks right". The window is told which
/// day to show through an event the Day view listens for.
pub fn open_main_window_on(app: &tauri::AppHandle, date: chrono::NaiveDate) {
    open_main_window(app);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit("open-day", date.to_string());
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
            job_status,
            job_state,
            engine_detect,
            refresh_engine_env,
            engine_test,
            engine_auth,
            get_settings,
            set_settings,
            set_launch_at_login,
            list_days,
            days_in_month,
            read_day,
            read_summary,
            open_in_editor,
            reveal_day,
            get_rules,
            add_rule,
            update_rule,
            remove_rule,
            get_prompt,
            set_prompt,
            reset_prompt,
            read_day_blocks,
            propose,
            apply_proposal,
            discard_proposal,
            copy_context,
            mcp_registration
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            app.manage(capture::CaptureState::new());
            app.manage(EngineEnv::default());
            {
                // Warm the login-shell environment off the launch path: it
                // costs about half a second and nothing needs it yet.
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    let _ = engine_env(&handle);
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
