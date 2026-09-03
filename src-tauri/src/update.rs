//! Update checks and installs, against the GitHub Releases `latest.json`
//! the updater plugin is configured with.
//!
//! Two entry points share one slot. The background check runs on its own
//! thread, shortly after launch and then every six hours, and never shows
//! a dialog: offline is normal, and a menu bar app that interrupts to say
//! it could not reach GitHub is worse than one that quietly tries again
//! later. When it finds a version it holds the `Update` in the slot, sends
//! one notification per version per launch, and relabels the tray item.
//! The tray item and the About button are the interactive path: they
//! install the held update if there is one, and otherwise run a check that
//! does report its outcome, because the user asked.
//!
//! The slot has three states rather than two because an install replaces
//! the bundle on disk. While that is happening a second install, or a
//! check that overwrites the offer, would race it; `Installing` refuses
//! both until the install has either relaunched the app or failed.
//!
//! The desktop notification plugin has no click handler, so the
//! notification can only announce; the install always happens from the
//! tray or About.

use crate::{capture, settings, tray};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
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

/// How often the background thread wakes to see whether a check is due
/// and still wanted. Short enough that turning the setting back on takes
/// effect within a minute rather than at the next six-hour mark.
const TICK: Duration = Duration::from_secs(30);

/// What the tray offers, derived from the slot for display and routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offer {
    None,
    Ready(String),
    Installing(String),
}

enum Held<T> {
    None,
    Ready(T, String),
    Installing(String),
}

/// The slot the checks fill and an install drains. Every transition takes
/// the lock once, so two threads cannot both believe they own the install.
/// Generic over the payload so the transitions can be tested without an
/// `Update`, which only the plugin can construct.
pub struct Slot<T> {
    held: Mutex<Held<T>>,
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Slot {
            held: Mutex::new(Held::None),
        }
    }
}

impl<T> Slot<T> {
    pub fn offer(&self) -> Offer {
        match self.held.lock() {
            Ok(held) => match &*held {
                Held::None => Offer::None,
                Held::Ready(_, version) => Offer::Ready(version.clone()),
                Held::Installing(version) => Offer::Installing(version.clone()),
            },
            Err(_) => Offer::None,
        }
    }

    /// A version a check found. Ignored while an install is running: the
    /// bundle on disk is about to change, and a fresh offer would race it.
    fn set_ready(&self, payload: T, version: &str) {
        if let Ok(mut held) = self.held.lock() {
            if !matches!(*held, Held::Installing(_)) {
                *held = Held::Ready(payload, version.to_string());
            }
        }
    }

    /// No newer version on the server. Ignored while installing, for the
    /// same reason as `set_ready`.
    fn clear(&self) {
        if let Ok(mut held) = self.held.lock() {
            if !matches!(*held, Held::Installing(_)) {
                *held = Held::None;
            }
        }
    }

    /// Claims the held update for an install. None when nothing is held
    /// or an install is already running; in both cases the caller has
    /// nothing to do.
    fn begin_ready(&self) -> Option<T> {
        let mut held = self.held.lock().ok()?;
        match std::mem::replace(&mut *held, Held::None) {
            Held::Ready(payload, version) => {
                *held = Held::Installing(version);
                Some(payload)
            }
            other => {
                *held = other;
                None
            }
        }
    }

    /// Claims the slot for an install of a payload the caller already
    /// holds (the one its dialog described). None when an install is
    /// already running.
    fn begin(&self, payload: T, version: &str) -> Option<T> {
        let mut held = self.held.lock().ok()?;
        if matches!(*held, Held::Installing(_)) {
            return None;
        }
        *held = Held::Installing(version.to_string());
        Some(payload)
    }

    /// An install that failed. The offer stands again whatever the slot
    /// says, because this thread is the one that was installing.
    fn fail(&self, payload: T, version: &str) {
        if let Ok(mut held) = self.held.lock() {
            *held = Held::Ready(payload, version.to_string());
        }
    }
}

/// The slot, the version already announced this launch, and whether an
/// interactive check is in flight. `notified` is what stops a six-hourly
/// check from announcing the same version again; `interactive` is what
/// stops a double-click on the tray or About from stacking dialogs.
#[derive(Default)]
pub struct UpdateState {
    slot: Slot<Update>,
    notified: Mutex<Option<String>>,
    interactive: AtomicBool,
}

impl UpdateState {
    pub fn offer(&self) -> Offer {
        self.slot.offer()
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

/// The tray item's label: an offer to install when a version is held, a
/// progress note while it installs, otherwise the standing macOS wording
/// for a manual check.
pub fn menu_label(offer: &Offer) -> String {
    match offer {
        Offer::None => "Check for Updates\u{2026}".to_string(),
        Offer::Ready(version) => format!("Update to {version}\u{2026}"),
        Offer::Installing(version) => format!("Installing {version}\u{2026}"),
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
        let mut next = Instant::now() + FIRST_CHECK;
        loop {
            std::thread::sleep(TICK);
            if Instant::now() < next {
                continue;
            }
            // A check that is due but not wanted stays due, so turning the
            // setting back on runs one within a tick.
            if !settings::load(&app).check_updates {
                continue;
            }
            check_quietly(&app);
            next = Instant::now() + CHECK_EVERY;
        }
    });
}

/// The tray item. Installs the held update if there is one, waits if one
/// is installing, and otherwise runs a check that reports what it found.
pub fn tray_action(app: &AppHandle) {
    match app.state::<UpdateState>().offer() {
        Offer::Ready(_) => {
            let app = app.clone();
            std::thread::spawn(move || install_ready(&app));
        }
        Offer::Installing(_) => {}
        Offer::None => check_interactive(app),
    }
}

/// A check the user asked for, so every outcome is shown: a version with
/// an Install / Later choice, the latest version, or the error. One at a
/// time: a second click while the first is still talking to GitHub would
/// otherwise stack a second dialog behind the first.
pub fn check_interactive(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<UpdateState>();
        if state.interactive.swap(true, Ordering::SeqCst) {
            return;
        }
        check_interactive_inner(&app);
        state.interactive.store(false, Ordering::SeqCst);
    });
}

fn check_interactive_inner(app: &AppHandle) {
    match fetch(app) {
        Ok(Some(update)) => {
            let version = update.version.clone();
            let state = app.state::<UpdateState>();
            if let Offer::Installing(installing) = state.offer() {
                message(
                    app,
                    MessageDialogKind::Info,
                    &format!("Version {installing} is installing now."),
                );
                return;
            }
            state.slot.set_ready(update.clone(), &version);
            // The dialog is the announcement; the background check must not
            // follow it with a notification for the same version.
            state.mark_notified(&version);
            refresh_tray(app);
            let chosen = app
                .dialog()
                .message(format!("Version {version} is available. Install it now?"))
                .title("Ambient Context")
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "Install".to_string(),
                    "Later".to_string(),
                ))
                .blocking_show();
            if !chosen {
                return;
            }
            // The update the dialog described, not whatever the slot holds
            // by now: a background check may have cleared it meanwhile, and
            // the answer to "install 1.0.1?" is an install of 1.0.1.
            match state.slot.begin(update, &version) {
                Some(update) => install(app, update),
                None => message(
                    app,
                    MessageDialogKind::Info,
                    "An update is already installing.",
                ),
            }
        }
        Ok(None) => {
            app.state::<UpdateState>().slot.clear();
            refresh_tray(app);
            message(
                app,
                MessageDialogKind::Info,
                "You're on the latest version.",
            );
        }
        Err(error) => message(
            app,
            MessageDialogKind::Error,
            &format!("Could not check for updates.\n{error}"),
        ),
    }
}

fn check_quietly(app: &AppHandle) {
    match fetch(app) {
        Ok(Some(update)) => {
            let version = update.version.clone();
            let state = app.state::<UpdateState>();
            state.slot.set_ready(update, &version);
            if state.mark_notified(&version) {
                notify(app, &notice_body(&version));
            }
            refresh_tray(app);
        }
        Ok(None) => {
            // A release pulled after it was found: the offer goes with it.
            app.state::<UpdateState>().slot.clear();
            refresh_tray(app);
        }
        Err(error) => eprintln!("[update] background check failed: {error}"),
    }
}

fn fetch(app: &AppHandle) -> Result<Option<Update>, String> {
    let updater = app.updater().map_err(|error| error.to_string())?;
    tauri::async_runtime::block_on(updater.check()).map_err(|error| error.to_string())
}

/// The tray's install: claims whatever the slot holds. Nothing held means
/// another click got there first (the tray now says so) or a check found
/// the release gone, and only the second deserves a message.
fn install_ready(app: &AppHandle) {
    let state = app.state::<UpdateState>();
    match state.slot.begin_ready() {
        Some(update) => install(app, update),
        None => {
            if state.offer() == Offer::None {
                refresh_tray(app);
                message(
                    app,
                    MessageDialogKind::Info,
                    "That update is no longer available.",
                );
            }
        }
    }
}

/// Downloads, verifies against the configured public key, installs and
/// relaunches. The slot is already `Installing` when this is called, and
/// the tray is refreshed so the item says so for the whole download. On
/// failure the offer is restored. Capture is stopped before the relaunch,
/// the same way Quit stops it, so the open block is flushed before the
/// process goes away.
fn install(app: &AppHandle, update: Update) {
    let version = update.version.clone();
    refresh_tray(app);
    let installed = tauri::async_runtime::block_on(update.download_and_install(|_, _| {}, || {}));
    if let Err(error) = installed {
        app.state::<UpdateState>().slot.fail(update, &version);
        refresh_tray(app);
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
    fn the_tray_item_names_each_state() {
        assert_eq!(menu_label(&Offer::None), "Check for Updates\u{2026}");
        assert_eq!(
            menu_label(&Offer::Ready("1.0.1".into())),
            "Update to 1.0.1\u{2026}"
        );
        assert_eq!(
            menu_label(&Offer::Installing("1.0.1".into())),
            "Installing 1.0.1\u{2026}"
        );
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
        assert_eq!(state.offer(), Offer::None);
    }

    #[test]
    fn a_found_version_is_offered_and_claimed_once() {
        let slot: Slot<&str> = Slot::default();
        slot.set_ready("bundle", "1.0.1");
        assert_eq!(slot.offer(), Offer::Ready("1.0.1".into()));
        assert_eq!(slot.begin_ready(), Some("bundle"));
        assert_eq!(slot.offer(), Offer::Installing("1.0.1".into()));
        // The second click of a double-click finds nothing to claim.
        assert_eq!(slot.begin_ready(), None);
    }

    #[test]
    fn checks_cannot_disturb_an_install_in_progress() {
        let slot: Slot<&str> = Slot::default();
        slot.set_ready("bundle", "1.0.1");
        slot.begin_ready();
        slot.set_ready("newer", "1.0.2");
        assert_eq!(slot.offer(), Offer::Installing("1.0.1".into()));
        slot.clear();
        assert_eq!(slot.offer(), Offer::Installing("1.0.1".into()));
        // The dialog's own install is refused too.
        assert_eq!(slot.begin("dialog", "1.0.1"), None);
    }

    #[test]
    fn a_failed_install_restores_the_offer() {
        let slot: Slot<&str> = Slot::default();
        slot.set_ready("bundle", "1.0.1");
        let claimed = slot.begin_ready().unwrap();
        slot.fail(claimed, "1.0.1");
        assert_eq!(slot.offer(), Offer::Ready("1.0.1".into()));
        assert_eq!(slot.begin_ready(), Some("bundle"));
    }

    #[test]
    fn the_dialog_installs_what_it_described_even_after_the_slot_cleared() {
        let slot: Slot<&str> = Slot::default();
        slot.set_ready("bundle", "1.0.1");
        slot.clear();
        assert_eq!(slot.offer(), Offer::None);
        assert_eq!(slot.begin("bundle", "1.0.1"), Some("bundle"));
        assert_eq!(slot.offer(), Offer::Installing("1.0.1".into()));
    }

    #[test]
    fn a_pulled_release_clears_a_standing_offer() {
        let slot: Slot<&str> = Slot::default();
        slot.set_ready("bundle", "1.0.1");
        slot.clear();
        assert_eq!(slot.begin_ready(), None);
    }
}
