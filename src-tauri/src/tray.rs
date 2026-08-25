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

/// Rebuilds the icon and the two status lines. Called after every state
/// change so the menu bar always reflects what is actually happening.
pub fn refresh(app: &AppHandle, capturing: bool, blocks: usize) {
    let state = if capturing {
        TrayState::Capturing
    } else if reader::macos::permission_status() == reader::Permission::NotGranted {
        TrayState::NeedsAttention
    } else {
        TrayState::Off
    };
    let _ = set_state(app, state);

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_menu(app, capturing, blocks) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

pub fn set_state(app: &AppHandle, state: TrayState) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_icon(Some(Image::from_bytes(state.icon_bytes())?))?;
        tray.set_icon_as_template(true)?;
    }
    Ok(())
}

fn status_text(capturing: bool, blocks: usize) -> String {
    if capturing {
        format!("Capturing \u{00b7} {blocks} blocks today")
    } else {
        "Not capturing".to_string()
    }
}

fn build_menu(app: &AppHandle, capturing: bool, blocks: usize) -> tauri::Result<Menu<tauri::Wry>> {
    let version = MenuItem::with_id(
        app,
        "version",
        format!("Ambient Context {}", env!("CARGO_PKG_VERSION")),
        false,
        None::<&str>,
    )?;
    let status = MenuItem::with_id(
        app,
        "status",
        status_text(capturing, blocks),
        false,
        None::<&str>,
    )?;
    let open_today = MenuItem::with_id(app, "open_today", "Open Today's File", true, None::<&str>)?;
    let reveal = MenuItem::with_id(app, "reveal", "Reveal Folder", true, None::<&str>)?;
    let change = MenuItem::with_id(
        app,
        "change_folder",
        "Change Folder\u{2026}",
        true,
        None::<&str>,
    )?;
    let setup = MenuItem::with_id(
        app,
        "setup",
        "Setup & Permissions\u{2026}",
        true,
        None::<&str>,
    )?;
    let updates = MenuItem::with_id(
        app,
        "updates",
        "Check for Updates\u{2026}",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    Menu::with_items(
        app,
        &[
            &version,
            &status,
            &PredefinedMenuItem::separator(app)?,
            &open_today,
            &reveal,
            &change,
            &PredefinedMenuItem::separator(app)?,
            &setup,
            &updates,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )
}

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app, false, 0)?;

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
                "change_folder" | "setup" => crate::open_setup_window(app),
                "updates" => crate::check_for_updates(app),
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
            let app = tray.app_handle().clone();
            let state = app.state::<capture::CaptureState>().inner().clone();

            // Permission missing or no folder chosen: open setup rather than
            // failing silently. This is the only path that opens a window
            // from a left click.
            if reader::macos::permission_status() == reader::Permission::NotGranted {
                crate::open_setup_window(&app);
                return;
            }
            let config = settings::load(&app);
            if config.folder.is_none() {
                crate::open_setup_window(&app);
                return;
            }

            if state.is_running() {
                capture::stop(&state);
                // The thread notices within ~100ms and flushes on its way
                // out, but the icon must empty now, not when it does.
                refresh(&app, false, state.blocks_today());
            } else {
                capture::start(app.clone(), &state, config);
                refresh(&app, true, state.blocks_today());
            }
        })
        .build(app)?;

    Ok(())
}
