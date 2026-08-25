#![cfg(target_os = "macos")]

use super::{parse_snapshot_json, permission_from_code, Permission, Snapshot};
use swift_rs::{swift, SRString};

swift!(fn ambient_ax_permission_status() -> i32);
swift!(fn ambient_ax_request_permission() -> i32);
swift!(fn ambient_ax_snapshot() -> SRString);

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
