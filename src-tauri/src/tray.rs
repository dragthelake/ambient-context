use crate::{capture, reader, settings};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

pub const TRAY_ID: &str = "main";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrayState {
    Off,
    Capturing,
    NeedsAttention,
}

impl TrayState {
    fn icon_bytes(self) -> &'static [u8] {
        match self {
            TrayState::Off => include_bytes!("../icons/tray-off.png"),
            TrayState::Capturing => include_bytes!("../icons/tray-on.png"),
            TrayState::NeedsAttention => include_bytes!("../icons/tray-alert.png"),
        }
    }
}

/// Rebuilds the icon and the toggle label. Called after every state change
/// so the menu bar always reflects what is actually happening. Marshalled
/// onto the main thread because the capture thread calls this too, and menu
/// construction on macOS is main-thread-only.
pub fn refresh(app: &AppHandle, capturing: bool) {
    let handle = app.clone();
    let queued = app.run_on_main_thread(move || {
        let state = if capturing {
            TrayState::Capturing
        } else if reader::macos::permission_status() == reader::Permission::NotGranted {
            TrayState::NeedsAttention
        } else {
            TrayState::Off
        };
        let _ = set_state(&handle, state);

        if let Some(tray) = handle.tray_by_id(TRAY_ID) {
            match build_menu(&handle, capturing) {
                Ok(menu) => {
                    if let Err(e) = tray.set_menu(Some(menu)) {
                        eprintln!("[tray] set_menu failed: {e}");
                    }
                }
                Err(e) => eprintln!("[tray] build_menu failed: {e}"),
            }
        }
    });
    if let Err(e) = queued {
        eprintln!("[tray] run_on_main_thread failed: {e}");
    }
}

pub fn set_state(app: &AppHandle, state: TrayState) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_icon(Some(Image::from_bytes(state.icon_bytes())?))?;
        tray.set_icon_as_template(true)?;
    }
    Ok(())
}

fn build_menu(app: &AppHandle, capturing: bool) -> tauri::Result<Menu<tauri::Wry>> {
    let version = MenuItem::with_id(
        app,
        "version",
        format!("Ambient Context {}", env!("CARGO_PKG_VERSION")),
        false,
        None::<&str>,
    )?;
    let toggle = MenuItem::with_id(
        app,
        "toggle",
        if capturing {
            "Stop Capturing"
        } else {
            "Start Capturing"
        },
        true,
        None::<&str>,
    )?;
    let last_run = {
        let text = app
            .try_state::<crate::jobs::JobState>()
            .and_then(|state| state.last_outcome())
            .map(|outcome| outcome.message)
            .unwrap_or_else(|| "No summaries yet".to_string());
        MenuItem::with_id(app, "last_run", text, false, None::<&str>)?
    };
    let open_today = MenuItem::with_id(app, "open_today", "Open Today's File", true, None::<&str>)?;
    let browse = MenuItem::with_id(app, "browse", "Browse Days\u{2026}", true, None::<&str>)?;
    let reveal = MenuItem::with_id(app, "reveal", "Reveal Folder", true, None::<&str>)?;
    let setup = MenuItem::with_id(app, "setup", "Settings\u{2026}", true, None::<&str>)?;
    let about = MenuItem::with_id(app, "about", "About\u{2026}", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    Menu::with_items(
        app,
        &[
            &version,
            &toggle,
            &last_run,
            &PredefinedMenuItem::separator(app)?,
            &open_today,
            &browse,
            &reveal,
            &PredefinedMenuItem::separator(app)?,
            &setup,
            &about,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )
}

/// Starts or stops capture. Shared by the Start/Stop Capturing menu item
/// and the settings page's switch, so the two can never drift apart.
/// Permission missing or no folder chosen: open setup rather than failing
/// silently.
pub fn toggle_capture(app: &AppHandle) {
    if reader::macos::permission_status() == reader::Permission::NotGranted {
        crate::open_setup_window(app);
        return;
    }
    let config = settings::load(app);
    if config.folder.is_none() {
        crate::open_setup_window(app);
        return;
    }

    let state = app.state::<capture::CaptureState>().inner().clone();
    let mut config = config;
    if state.is_running() {
        capture::stop(&state);
        // An explicit stop is remembered: the app will not auto-start
        // capture again until the user starts it.
        config.enabled = false;
        record_toggle(app, &config);
        // The thread notices within ~100ms and flushes on its way
        // out, but the icon must empty now, not when it does.
        refresh(app, false);
    } else {
        config.enabled = true;
        record_toggle(app, &config);
        capture::start(app.clone(), &state, config);
        refresh(app, true);
    }
}

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app, false)?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(TrayState::Off.icon_bytes())?)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let config = settings::load(app);
            match event.id().as_ref() {
                "open_today" => {
                    if let Some(folder) = &config.folder {
                        let path =
                            crate::writer::file_path(folder, chrono::Local::now().date_naive());
                        let _ = std::process::Command::new("open").arg(path).spawn();
                    }
                }
                "reveal" => {
                    if let Some(folder) = &config.folder {
                        let _ = std::process::Command::new("open").arg(folder).spawn();
                    }
                }
                "toggle" => toggle_capture(app),
                "browse" => crate::open_main_window(app),
                "setup" => crate::open_setup_window(app),
                "about" => crate::open_about_window(app),
                "quit" => {
                    let state = app.state::<capture::CaptureState>();
                    capture::stop(&state);
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            else {
                return;
            };
            crate::open_main_window(tray.app_handle());
        })
        .build(app)?;

    Ok(())
}

/// The capture toggle writes `enabled` to settings.json, and a settings
/// write is a settings write whichever surface made it: it goes through the
/// same recorded save as the Settings page and MCP.
fn record_toggle(app: &tauri::AppHandle, config: &settings::Settings) {
    if let Err(error) = crate::save_settings_recorded(
        &settings::config_dir(app),
        config.folder.as_deref(),
        "toggle_capture",
        config,
    ) {
        eprintln!("[settings] could not record the capture toggle: {error}");
    }
}
