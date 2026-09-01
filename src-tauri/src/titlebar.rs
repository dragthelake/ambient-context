//! The main window's chrome. macOS draws three traffic light buttons on
//! every decorated window, and this app draws its own close button in a
//! Windows 98 title bar, so the native three are hidden. Going borderless
//! would hide them too, but it would also give up the system corner mask
//! and edge resizing, which are both wanted.

#[cfg(target_os = "macos")]
use tauri::WebviewWindow;

#[cfg(target_os = "macos")]
swift_rs::swift!(fn ambient_hide_window_buttons(window: i64) -> i32);

/// Hides the traffic lights on a window. Safe to call more than once: the
/// window is fetched fresh and hiding an already hidden button is a no-op.
#[cfg(target_os = "macos")]
pub fn hide_traffic_lights(window: &WebviewWindow) {
    // The pointer is read here and moved as an integer, because a raw
    // pointer is not Send and this has to finish on the main thread:
    // AppKit will not touch window chrome from anywhere else.
    let Ok(ns_window) = window.ns_window() else {
        eprintln!("[titlebar] no NSWindow for this window; leaving its buttons alone");
        return;
    };
    let address = ns_window as i64;
    let queued = window.run_on_main_thread(move || {
        let hidden = unsafe { ambient_hide_window_buttons(address) };
        if hidden != 3 {
            eprintln!("[titlebar] hid {hidden} of 3 traffic lights");
        }
    });
    if let Err(error) = queued {
        eprintln!("[titlebar] could not reach the main thread: {error}");
    }
}

#[cfg(not(target_os = "macos"))]
pub fn hide_traffic_lights(_window: &tauri::WebviewWindow) {}
