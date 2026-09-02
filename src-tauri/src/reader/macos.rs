#![cfg(target_os = "macos")]

use super::{parse_snapshot_json, permission_from_code, Permission, Snapshot};
use swift_rs::{swift, SRString};

swift!(fn ambient_ax_permission_status() -> i32);
swift!(fn ambient_ax_request_permission() -> i32);
swift!(fn ambient_ax_snapshot() -> SRString);
swift!(fn ambient_ax_seconds_since_input() -> f64);

pub fn permission_status() -> Permission {
    permission_from_code(unsafe { ambient_ax_permission_status() })
}

pub fn request_permission() -> Permission {
    permission_from_code(unsafe { ambient_ax_request_permission() })
}

pub fn snapshot() -> Option<Snapshot> {
    let raw = unsafe { ambient_ax_snapshot() };
    match parse_snapshot_json(raw.as_str()) {
        Ok(snapshot) => Some(snapshot),
        Err(message) => {
            eprintln!("[ax] snapshot failed: {message}");
            None
        }
    }
}

/// Negative means Swift could not read the idle time; the caller must not
/// treat that as a fresh keystroke, so it becomes None.
pub fn seconds_since_input() -> Option<f64> {
    let seconds = unsafe { ambient_ax_seconds_since_input() };
    (seconds >= 0.0).then_some(seconds)
}
