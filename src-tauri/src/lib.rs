mod capture;
mod engine;
mod prune;
mod reader;
mod redact;
mod segment;
mod settings;
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

pub fn open_setup_window(app: &tauri::AppHandle) {
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
            open_link
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            app.manage(capture::CaptureState::new());
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
