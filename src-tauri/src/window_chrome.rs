//! macOS window chrome helpers. The main window keeps the native traffic
//! lights and draws its own title bar under them; this module lines the
//! two up without resizing AppKit's title bar container.

#[cfg(target_os = "macos")]
use tauri::WebviewWindow;

#[cfg(target_os = "macos")]
swift_rs::swift!(fn ambient_position_traffic_lights(
    window: i64,
    x: f64,
    y_from_top: f64
) -> i32);

/// Lines the native traffic lights up with the page-drawn title bar.
#[cfg(target_os = "macos")]
pub fn centre_traffic_lights(window: &WebviewWindow) {
    // Measured against main-window.css: 5px window border, 28px title bar,
    // ~12px traffic-light diameter. x is a few pixels right of the default.
    const X: f64 = 15.0;
    const BORDER: f64 = 5.0;
    const TITLE_BAR: f64 = 28.0;
    const BUTTON: f64 = 12.0;
    const Y_FROM_TOP: f64 = BORDER + (TITLE_BAR - BUTTON) / 2.0;

    let Ok(ns_window) = window.ns_window() else {
        eprintln!("[window_chrome] no NSWindow for this window; leaving traffic lights alone");
        return;
    };
    let address = ns_window as i64;
    let queued = window.run_on_main_thread(move || {
        let moved = unsafe { ambient_position_traffic_lights(address, X, Y_FROM_TOP) };
        if moved != 3 {
            eprintln!("[window_chrome] repositioned {moved} of 3 traffic lights");
        }
    });
    if let Err(error) = queued {
        eprintln!("[window_chrome] could not reach the main thread: {error}");
    }
}

#[cfg(not(target_os = "macos"))]
pub fn centre_traffic_lights(_window: &tauri::WebviewWindow) {}
