use crate::{
    prune,
    reader::{self, Snapshot, WindowReader},
    redact, rules,
    segment::Segmenter,
    settings::{self, Settings},
    writer,
};
use chrono::{Local, NaiveDate};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;
use tauri::AppHandle;

fn rules_mtime(config_dir: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(crate::rules::rules_path(config_dir))
        .and_then(|m| m.modified())
        .ok()
}

/// The capture target must never include the capture output: reading
/// today's file re-captures the previous blocks, and the dwell segmenter
/// then emits a session whose content is the earlier sessions. Matched at
/// emit time against the configured folder and today's filename.
fn is_own_output(snapshot: &Snapshot, folder: &Path, today: NaiveDate) -> bool {
    let folder_str = folder.to_string_lossy();
    if snapshot
        .document
        .as_deref()
        .is_some_and(|d| d.contains(folder_str.as_ref()))
    {
        return true;
    }
    if snapshot
        .url
        .as_deref()
        .is_some_and(|u| u.contains(folder_str.as_ref()))
    {
        return true;
    }
    if let Some(title) = &snapshot.window_title {
        let stem = today.format("%Y-%m-%d").to_string();
        if title.contains(&format!("{stem}.md")) {
            return true;
        }
        if let Some(name) = folder.file_name() {
            let name = name.to_string_lossy();
            if title.contains(&stem) && title.contains(name.as_ref()) {
                return true;
            }
        }
    }
    false
}

/// How long `stop` will wait for the poll thread to leave. The thread
/// checks the flag every 100ms, except while it is inside an accessibility
/// read, and the reader's own messaging timeout bounds that. Waiting
/// longer than this would mean the read is wedged, and starting a second
/// thread beside a wedged one is the failure worth avoiding.
pub const STOP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Default)]
pub struct CaptureState {
    running: Arc<AtomicBool>,
    blocks_today: Arc<AtomicUsize>,
    /// How many poll threads are alive. `stop` waits on this rather than on
    /// a sleep, because the flag going false says only that the thread has
    /// been asked to leave, not that it has.
    live: Arc<(Mutex<usize>, Condvar)>,
}

impl CaptureState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn blocks_today(&self) -> usize {
        self.blocks_today.load(Ordering::SeqCst)
    }
}

/// Spawns the poll thread. Returns immediately. Calling this while already
/// running is a no-op rather than a second thread.
pub fn start(app: AppHandle, state: &CaptureState, settings: Settings) {
    let Some(mut folder) = settings.folder.clone() else {
        eprintln!("[capture] refusing to start with no folder set");
        return;
    };

    let counter = state.blocks_today.clone();

    let spawned = spawn_tracked(state, move |running| {
        let mut segmenter = Segmenter::new(settings.min_dwell_secs, settings.similarity_threshold);
        let mut dedup = writer::DayDedup::new();
        let interval = Duration::from_secs(settings.interval_secs.max(1));
        let mut failed_reads: u32 = 0;
        let mut counter_day = Local::now().date_naive();
        let config_dir = settings::config_dir(&app);
        let mut rules = rules::load(&config_dir);
        let mut rules_stamp = rules_mtime(&config_dir);
        let mut extra = redact::compile_extra(&settings.extra_redaction_patterns);
        let mut extra_source = settings.extra_redaction_patterns.clone();
        // The two output knobs the settings page exposes; rebuilt whenever
        // the settings they come from change.
        let mut shape = writer::Shape {
            max_block_chars: settings.max_block_chars,
            write_references: settings.write_references,
        };

        while running.load(Ordering::SeqCst) {
            // "47 blocks today" must mean today. Reset the count when the
            // date rolls over rather than letting it accumulate forever.
            let today = Local::now().date_naive();
            if today != counter_day {
                counter_day = today;
                counter.store(0, Ordering::SeqCst);
            }

            // A folder change in settings takes effect live, without
            // restarting capture. The open block belongs to the old folder;
            // flush it there before switching, and forget what the old
            // folder had seen.
            if let Some(new_folder) = settings::load(&app).folder {
                if new_folder != folder {
                    if let Some(block) = segmenter.flush(Local::now()) {
                        let _ = writer::append_block(&folder, &block, &mut dedup, shape);
                    }
                    folder = new_folder;
                    dedup.reset();
                }
            }

            // Rules edited in Settings, by an agent over MCP, or in a text
            // editor take effect on the next poll rather than the next
            // launch. Compared by modification time so an unchanged file
            // costs one stat call per poll.
            let stamp = rules_mtime(&config_dir);
            if stamp != rules_stamp {
                rules_stamp = stamp;
                rules = rules::load(&config_dir);
            }
            let current = settings::load(&app);
            if current.extra_redaction_patterns != extra_source {
                extra = redact::compile_extra(&current.extra_redaction_patterns);
                extra_source = current.extra_redaction_patterns.clone();
            }
            let next_shape = writer::Shape {
                max_block_chars: current.max_block_chars,
                write_references: current.write_references,
            };
            if next_shape != shape {
                shape = next_shape;
            }

            match reader::PlatformReader.snapshot() {
                Some(raw) => {
                    failed_reads = 0;
                    if let Some(clean) = redact::redact_snapshot(raw, &rules, &extra) {
                        if is_own_output(&clean, &folder, today) {
                            // Looking at the capture file is not work worth
                            // recording, and recording it recurses.
                        } else {
                            let clean = Snapshot {
                                text: clean
                                    .text
                                    .iter()
                                    .filter_map(|line| prune::normalise_line(line))
                                    .collect(),
                                ..clean
                            };
                            if let Some(block) = segmenter.push(clean, Local::now()) {
                                match writer::append_block(&folder, &block, &mut dedup, shape) {
                                    Ok(()) => {
                                        counter.fetch_add(1, Ordering::SeqCst);
                                    }
                                    Err(e) => eprintln!("[capture] write failed: {e}"),
                                }
                            }
                        }
                    }
                }
                None => {
                    // A locked screen, a hung target or a dropped read.
                    // Three misses in a row closes the open block, so a
                    // block ends at the lock rather than spanning lunch.
                    failed_reads += 1;
                    if failed_reads == 3 {
                        if let Some(block) = segmenter.flush(Local::now()) {
                            if writer::append_block(&folder, &block, &mut dedup, shape).is_ok() {
                                counter.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                    }
                }
            }

            // Sleep in short slices so stop and quit take effect within
            // about 100ms. A single full-interval sleep means quitting
            // mid-sleep outlasts the 300ms grace in the quit handler and
            // silently loses the open block.
            let mut slept = Duration::ZERO;
            let slice = Duration::from_millis(100);
            while slept < interval && running.load(Ordering::SeqCst) {
                thread::sleep(slice);
                slept += slice;
            }
        }

        // Closing the open block on stop means the last stretch of work is
        // not silently lost when capture is switched off.
        if let Some(block) = segmenter.flush(Local::now()) {
            if writer::append_block(&folder, &block, &mut dedup, shape).is_ok() {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        }
        crate::tray::refresh(&app, false);
    });
    if !spawned {
        eprintln!("[capture] a poll thread is already running");
    }
}

/// Marks the state running and puts `body` on its own thread, counting it
/// in and out so `stop` can wait for it. Returns false when a thread is
/// already live, which is what makes a second start a no-op.
fn spawn_tracked<F>(state: &CaptureState, body: F) -> bool
where
    F: FnOnce(Arc<AtomicBool>) + Send + 'static,
{
    if state
        .running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return false;
    }
    let (count, _) = &*state.live;
    *count.lock().expect("capture thread count") += 1;
    let running = state.running.clone();
    let live = state.live.clone();
    thread::spawn(move || {
        body(running);
        let (count, exited) = &*live;
        *count.lock().expect("capture thread count") -= 1;
        exited.notify_all();
    });
    true
}

/// Asks the poll thread to leave and returns only once it has, or once
/// `STOP_TIMEOUT` expires. False means it is still running, and the caller
/// must not start another: two threads with their own segmenters append to
/// the same day file.
pub fn stop(state: &CaptureState) -> bool {
    state.running.store(false, Ordering::SeqCst);
    wait_until_stopped(state, STOP_TIMEOUT)
}

fn wait_until_stopped(state: &CaptureState, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    let (count, exited) = &*state.live;
    let mut live = count.lock().expect("capture thread count");
    while *live > 0 {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            eprintln!("[capture] the poll thread did not exit within {timeout:?}");
            return false;
        }
        let (guard, result) = exited
            .wait_timeout(live, remaining)
            .expect("capture thread count");
        live = guard;
        if result.timed_out() && *live > 0 {
            eprintln!("[capture] the poll thread did not exit within {timeout:?}");
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_state_is_not_running_and_has_no_blocks() {
        let state = CaptureState::new();
        assert!(!state.is_running());
        assert_eq!(state.blocks_today(), 0);
    }

    #[test]
    fn stop_is_idempotent() {
        let state = CaptureState::new();
        assert!(stop(&state));
        assert!(stop(&state));
        assert!(!state.is_running());
    }

    #[test]
    fn stop_returns_only_once_a_slow_poll_thread_has_left() {
        // The real reader can park the poll thread inside one snapshot;
        // this one parks it for 300ms before it looks at the flag again. A
        // restart on a 150ms timer would start a second thread inside that
        // window.
        let state = CaptureState::new();
        let reads = Arc::new(AtomicUsize::new(0));
        let counter = reads.clone();
        assert!(spawn_tracked(&state, move |running| loop {
            thread::sleep(Duration::from_millis(300));
            counter.fetch_add(1, Ordering::SeqCst);
            if !running.load(Ordering::SeqCst) {
                break;
            }
        }));

        let started = std::time::Instant::now();
        assert!(stop(&state), "the thread left within the timeout");
        assert!(
            started.elapsed() >= Duration::from_millis(250),
            "stop waited for the blocked read, not a fixed sleep; it took {:?}",
            started.elapsed()
        );
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        assert!(!state.is_running());

        // And having waited, a start is safe. Two starts still run one
        // thread: the second is refused rather than doubling the writer.
        let bodies = Arc::new(AtomicUsize::new(0));
        let first = bodies.clone();
        let second = bodies.clone();
        assert!(spawn_tracked(&state, move |_| {
            first.fetch_add(1, Ordering::SeqCst);
        }));
        assert!(
            !spawn_tracked(&state, move |_| {
                second.fetch_add(1, Ordering::SeqCst);
            }),
            "a second start while one is live is refused"
        );
        assert!(stop(&state));
        assert_eq!(bodies.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_stop_that_times_out_says_so_rather_than_pretending() {
        let state = CaptureState::new();
        assert!(spawn_tracked(&state, |running| {
            // Ignores the flag, the way a wedged accessibility read does.
            let _ = running;
            thread::sleep(Duration::from_millis(600));
        }));
        state.running.store(false, Ordering::SeqCst);
        assert!(!wait_until_stopped(&state, Duration::from_millis(50)));
        assert!(wait_until_stopped(&state, Duration::from_secs(5)));
    }

    #[test]
    fn clones_share_the_same_underlying_state() {
        let state = CaptureState::new();
        let clone = state.clone();
        state.running.store(true, Ordering::SeqCst);
        assert!(clone.is_running());
    }

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 8, 25).unwrap()
    }

    #[test]
    fn own_output_is_recognised_by_document_path() {
        let snap = Snapshot {
            app: "Obsidian".to_string(),
            document: Some("/Users/x/Ambient Context/2026-08-25.md".to_string()),
            ..Default::default()
        };
        assert!(is_own_output(
            &snap,
            Path::new("/Users/x/Ambient Context"),
            day()
        ));
    }

    #[test]
    fn own_output_is_recognised_by_todays_filename_in_the_title() {
        let snap = Snapshot {
            app: "TextEdit".to_string(),
            window_title: Some("2026-08-25.md".to_string()),
            ..Default::default()
        };
        assert!(is_own_output(
            &snap,
            Path::new("/Users/x/Ambient Context"),
            day()
        ));
    }

    #[test]
    fn own_output_is_recognised_by_stem_plus_folder_name_in_the_title() {
        let snap = Snapshot {
            app: "Obsidian".to_string(),
            window_title: Some("2026-08-25 - Ambient Context - Obsidian".to_string()),
            ..Default::default()
        };
        assert!(is_own_output(
            &snap,
            Path::new("/Users/x/Ambient Context"),
            day()
        ));
    }

    #[test]
    fn other_dated_documents_are_not_own_output() {
        let snap = Snapshot {
            app: "Obsidian".to_string(),
            window_title: Some("2026-08-23 - Audio Capture Spike Findings".to_string()),
            ..Default::default()
        };
        assert!(!is_own_output(
            &snap,
            Path::new("/Users/x/Ambient Context"),
            day()
        ));
    }

    #[test]
    fn ordinary_windows_are_not_own_output() {
        let snap = Snapshot {
            app: "Chrome".to_string(),
            window_title: Some("Tauri tray documentation".to_string()),
            url: Some("https://v2.tauri.app/".to_string()),
            ..Default::default()
        };
        assert!(!is_own_output(
            &snap,
            Path::new("/Users/x/Ambient Context"),
            day()
        ));
    }

    #[test]
    fn a_summary_the_app_wrote_is_own_output() {
        let snap = Snapshot {
            app: "Obsidian".to_string(),
            document: Some("/Users/x/Ambient Context/Summaries/2026-08-25.md".to_string()),
            ..Default::default()
        };
        assert!(is_own_output(
            &snap,
            Path::new("/Users/x/Ambient Context"),
            day()
        ));
    }

    #[test]
    fn a_ledger_entry_the_app_wrote_is_own_output() {
        let snap = Snapshot {
            app: "TextEdit".to_string(),
            document: Some("/Users/x/Ambient Context/Ledger/2026-08-25.md".to_string()),
            ..Default::default()
        };
        assert!(is_own_output(
            &snap,
            Path::new("/Users/x/Ambient Context"),
            day()
        ));
    }
}
