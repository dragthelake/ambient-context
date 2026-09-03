//! Update checks and installs, against the GitHub Releases `latest.json`
//! the updater plugin is configured with.
//!
//! Two entry points share one piece of state. The background check runs
//! on its own thread, shortly after launch and then every six hours, and
//! never shows a dialog: offline is normal, and a menu bar app that
//! interrupts to say it could not reach GitHub is worse than one that
//! quietly tries again later. When it finds a version it holds the
//! `Update`, sends one notification per version per launch, and relabels
//! the tray item. The tray item and the About button are the interactive
//! path: they install the held update if there is one, and otherwise run
//! a check that does report its outcome, because the user asked.
//!
//! The desktop notification plugin has no click handler, so the
//! notification can only announce; the install always happens from the
//! tray or About.

use crate::{capture, settings, tray};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::{Update, UpdaterExt};

/// Delay before the first background check. Launch is busy warming the
/// agent environment and starting capture; a version check can wait.
const FIRST_CHECK: Duration = Duration::from_secs(30);

/// Interval between background checks while the app is running. Releases
/// are days apart, so six hours finds one the same day without polling
/// GitHub for no reason.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// The update the last check found, and the version already announced
/// this launch. `pending` is what the tray item installs; `notified` is
/// what stops a six-hourly check from announcing the same version again.
#[derive(Default)]
pub struct UpdateState {
    pending: Mutex<Option<Update>>,
    notified: Mutex<Option<String>>,
}

impl UpdateState {
    pub fn pending_version(&self) -> Option<String> {
        self.pending
            .lock()
            .ok()?
            .as_ref()
            .map(|update| update.version.clone())
    }

    fn set_pending(&self, update: Option<Update>) {
        if let Ok(mut slot) = self.pending.lock() {
            *slot = update;
        }
    }

    fn take_pending(&self) -> Option<Update> {
        self.pending.lock().ok()?.take()
    }

    /// Records that `version` has been announced. True when it had not
    /// been, which is when the caller should announce it.
    fn mark_notified(&self, version: &str) -> bool {
        let Ok(mut slot) = self.notified.lock() else {
            return false;
        };
        if !should_notify(slot.as_deref(), version) {
            return false;
        }
        *slot = Some(version.to_string());
        true
    }
}

/// The tray item's label: an offer to install when a version is held,
/// otherwise the standing macOS wording for a manual check.
pub fn menu_label(pending: Option<&str>) -> String {
    match pending {
        Some(version) => format!("Update to {version}\u{2026}"),
        None => "Check for Updates\u{2026}".to_string(),
    }
}

/// One announcement per version. A check that finds the same version
/// again six hours later says nothing; a newer one is announced again.
pub fn should_notify(notified: Option<&str>, version: &str) -> bool {
    notified != Some(version)
}

pub fn notice_body(version: &str) -> String {
    format!("Version {version} is available. Update from the menu bar.")
}

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(FIRST_CHECK);
        loop {
            if settings::load(&app).check_updates {
                check_quietly(&app);
            }
            std::thread::sleep(CHECK_EVERY);
        }
    });
}

/// The tray item. Installs the held update if there is one, otherwise
/// runs a check that reports what it found.
pub fn tray_action(app: &AppHandle) {
    if app.state::<UpdateState>().pending_version().is_some() {
        let app = app.clone();
        std::thread::spawn(move || install_pending(&app));
    } else {
        check_interactive(app);
    }
}

/// A check the user asked for, so every outcome is shown: a version with
/// an Install / Later choice, the latest version, or the error.
pub fn check_interactive(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || match fetch(&app) {
        Ok(Some(update)) => {
            let version = update.version.clone();
            let state = app.state::<UpdateState>();
            state.set_pending(Some(update));
            // The dialog is the announcement; the background check must not
            // follow it with a notification for the same version.
            state.mark_notified(&version);
            refresh_tray(&app);
            let install = app
                .dialog()
                .message(format!("Version {version} is available. Install it now?"))
                .title("Ambient Context")
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "Install".to_string(),
                    "Later".to_string(),
                ))
                .blocking_show();
            if install {
                install_pending(&app);
            }
        }
        Ok(None) => {
            app.state::<UpdateState>().set_pending(None);
            refresh_tray(&app);
            message(
                &app,
                MessageDialogKind::Info,
                "You're on the latest version.",
            );
        }
        Err(error) => message(
            &app,
            MessageDialogKind::Error,
            &format!("Could not check for updates.\n{error}"),
        ),
    });
}

fn check_quietly(app: &AppHandle) {
    match fetch(app) {
        Ok(Some(update)) => {
            let version = update.version.clone();
            let state = app.state::<UpdateState>();
            state.set_pending(Some(update));
            if state.mark_notified(&version) {
                notify(app, &notice_body(&version));
            }
            refresh_tray(app);
        }
        Ok(None) => {
            // A release pulled after it was found: the offer goes with it.
            app.state::<UpdateState>().set_pending(None);
            refresh_tray(app);
        }
        Err(error) => eprintln!("[update] background check failed: {error}"),
    }
}

fn fetch(app: &AppHandle) -> Result<Option<Update>, String> {
    let updater = app.updater().map_err(|error| error.to_string())?;
    tauri::async_runtime::block_on(updater.check()).map_err(|error| error.to_string())
}

/// Downloads, verifies against the configured public key, installs and
/// relaunches. Capture is stopped first, the same way Quit does it, so the
/// open block is flushed before the process goes away.
fn install_pending(app: &AppHandle) {
    let Some(update) = app.state::<UpdateState>().take_pending() else {
        return;
    };
    let installed = tauri::async_runtime::block_on(update.download_and_install(|_, _| {}, || {}));
    if let Err(error) = installed {
        // Still offered: a download that failed once may work next time.
        app.state::<UpdateState>().set_pending(Some(update));
        message(
            app,
            MessageDialogKind::Error,
            &format!("The update failed.\n{error}"),
        );
        return;
    }
    let state = app.state::<capture::CaptureState>();
    capture::stop(&state);
    std::thread::sleep(Duration::from_millis(300));
    app.restart();
}

fn refresh_tray(app: &AppHandle) {
    let capturing = app.state::<capture::CaptureState>().is_running();
    tray::refresh(app, capturing);
}

fn notify(app: &AppHandle, body: &str) {
    if let Err(error) = app
        .notification()
        .builder()
        .title("Ambient Context")
        .body(body)
        .show()
    {
        eprintln!("[update] could not show the notification: {error}");
    }
}

fn message(app: &AppHandle, kind: MessageDialogKind, text: &str) {
    app.dialog()
        .message(text)
        .title("Ambient Context")
        .kind(kind)
        .blocking_show();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tray_item_offers_a_check_until_a_version_is_held() {
        assert_eq!(menu_label(None), "Check for Updates\u{2026}");
        assert_eq!(menu_label(Some("1.0.1")), "Update to 1.0.1\u{2026}");
    }

    #[test]
    fn a_version_is_announced_once_and_a_newer_one_again() {
        assert!(should_notify(None, "1.0.1"));
        assert!(!should_notify(Some("1.0.1"), "1.0.1"));
        assert!(should_notify(Some("1.0.1"), "1.0.2"));
    }

    #[test]
    fn the_notice_says_where_to_install_from() {
        assert_eq!(
            notice_body("1.0.1"),
            "Version 1.0.1 is available. Update from the menu bar."
        );
    }

    #[test]
    fn the_state_announces_each_version_once() {
        let state = UpdateState::default();
        assert!(state.mark_notified("1.0.1"));
        assert!(!state.mark_notified("1.0.1"));
        assert!(state.mark_notified("1.0.2"));
        assert_eq!(state.pending_version(), None);
    }
}
