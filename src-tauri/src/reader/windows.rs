#![cfg(target_os = "windows")]

use super::{Permission, Snapshot};

/// Windows UI Automation requires no consent, so permission is always
/// granted. The reader itself lands in a later release.
pub fn permission_status() -> Permission {
    Permission::Granted
}

pub fn request_permission() -> Permission {
    Permission::Granted
}

pub fn snapshot() -> Option<Snapshot> {
    None
}

/// No idle reading yet, so the idle check never fires on Windows.
pub fn seconds_since_input() -> Option<f64> {
    None
}
