use crate::{
    redact,
    reader::{self, WindowReader},
    segment::Segmenter,
    settings::Settings,
    writer,
};
use chrono::Local;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::AppHandle;

#[derive(Clone, Default)]
pub struct CaptureState {
    running: Arc<AtomicBool>,
    blocks_today: Arc<AtomicUsize>,
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
    if state
        .running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    let Some(folder) = settings.folder.clone() else {
        state.running.store(false, Ordering::SeqCst);
        eprintln!("[capture] refusing to start with no folder set");
        return;
    };

    let running = state.running.clone();
    let counter = state.blocks_today.clone();

    thread::spawn(move || {
        let mut segmenter = Segmenter::new(settings.min_dwell_secs, settings.similarity_threshold);
        let interval = Duration::from_secs(settings.interval_secs.max(1));
        let mut failed_reads: u32 = 0;
        let mut counter_day = Local::now().date_naive();

        while running.load(Ordering::SeqCst) {
            // "47 blocks today" must mean today. Reset the count when the
            // date rolls over rather than letting it accumulate forever.
            let today = Local::now().date_naive();
            if today != counter_day {
                counter_day = today;
                counter.store(0, Ordering::SeqCst);
            }

            match reader::PlatformReader.snapshot() {
                Some(raw) => {
                    failed_reads = 0;
                    if let Some(clean) = redact::redact_snapshot(raw) {
                        if let Some(block) = segmenter.push(clean, Local::now()) {
                            match writer::append_block(&folder, &block) {
                                Ok(()) => {
                                    counter.fetch_add(1, Ordering::SeqCst);
                                    crate::tray::refresh(&app, true, counter.load(Ordering::SeqCst));
                                }
                                Err(e) => eprintln!("[capture] write failed: {e}"),
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
                            if writer::append_block(&folder, &block).is_ok() {
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
            if writer::append_block(&folder, &block).is_ok() {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        }
        crate::tray::refresh(&app, false, counter.load(Ordering::SeqCst));
    });
}

pub fn stop(state: &CaptureState) {
    state.running.store(false, Ordering::SeqCst);
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
        stop(&state);
        stop(&state);
        assert!(!state.is_running());
    }

    #[test]
    fn clones_share_the_same_underlying_state() {
        let state = CaptureState::new();
        let clone = state.clone();
        state.running.store(true, Ordering::SeqCst);
        assert!(clone.is_running());
    }
}
