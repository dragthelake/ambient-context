mod capture;
mod days;
mod engine;
mod jobs;
mod ledger;
mod prune;
mod reader;
mod redact;
mod segment;
mod settings;
mod summarise;
mod tray;
mod writer;

use serde::Serialize;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
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
    let sample: String = snap
        .text
        .join(" ")
        .chars()
        .take(200)
        .collect();
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
    let folder = settings::load(&app).folder.ok_or("no capture folder is set")?;
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

/// Runs on the async pool: the engine can take minutes and this must not
/// block the webview or the main thread.
#[tauri::command]
async fn summarise_now(app: tauri::AppHandle, date: String) -> Result<(), String> {
    let parsed = parse_date(&date)?;
    tauri::async_runtime::spawn_blocking(move || {
        let result = jobs::run_one(&app, parsed, ledger::Trigger::OnDemand);
        crate::tray::refresh(&app, app.state::<capture::CaptureState>().is_running());
        result
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn job_status(state: tauri::State<jobs::JobState>) -> Option<jobs::Outcome> {
    state.last_outcome()
}

#[tauri::command]
fn engine_detect() -> Vec<settings::Engine> {
    engine::detect(&engine::login_shell_env())
}

/// Proves the connection now, in front of someone who can fix it, rather
/// than at 6am six weeks from now.
#[tauri::command]
async fn engine_test(engine_config: settings::Engine) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut probe = engine_config;
        // A test must never park the window for ten minutes.
        probe.timeout_secs = probe.timeout_secs.min(60);
        engine::run(
            &probe,
            "Reply with exactly the word: ok",
            &engine::login_shell_env(),
        )
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Whether the engine is signed in, without spending a model call. Ten
/// second cap inside `auth_state`; never called on the schedule.
#[tauri::command]
async fn engine_auth(engine_config: settings::Engine) -> engine::AuthState {
    tauri::async_runtime::spawn_blocking(move || {
        engine::auth_state(&engine_config, &engine::login_shell_env())
    })
    .await
    .unwrap_or(engine::AuthState::Unknown)
}

#[tauri::command]
fn get_settings(app: tauri::AppHandle) -> settings::Settings {
    settings::load(&app)
}

#[tauri::command]
fn set_settings(app: tauri::AppHandle, next: settings::Settings) -> Result<(), String> {
    settings::save(&app, &next).map_err(|e| e.to_string())
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

    #[test]
    fn a_bad_date_is_an_error_rather_than_a_panic() {
        assert!(parse_date("not-a-date").is_err());
        assert!(parse_date("2026-08-28").is_ok());
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
    if let Ok(window) = WebviewWindowBuilder::new(
        app,
        "main",
        WebviewUrl::App("index.html?view=main".into()),
    )
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
            engine_detect,
            engine_test,
            engine_auth,
            get_settings,
            set_settings,
            list_days,
            days_in_month,
            read_day,
            read_summary,
            open_in_editor,
            reveal_day
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            app.manage(capture::CaptureState::new());
            app.manage(jobs::JobState::new());
            jobs::start(app.handle().clone());
            tray::build(app.handle())?;

            let config = settings::load(app.handle());
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
