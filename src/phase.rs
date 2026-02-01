use std::sync::atomic::{AtomicBool, Ordering};

/// Current phase of the benchmark execution.
#[derive(Clone, Debug, Default)]
pub enum BenchPhase {
    /// Waiting to start.
    #[default]
    Pending,
    /// Workers initializing state and running setup.
    Setup {
        /// Number of workers that have completed setup.
        completed: usize,
        /// Total number of workers.
        total: usize,
    },
    /// Running warmup iterations (results discarded).
    Warmup {
        /// Number of warmup iterations completed.
        completed: u64,
        /// Total number of warmup iterations.
        total: u64,
    },
    /// Main benchmark phase.
    Bench,
}

/// Current run state of the benchmark.
///
/// State transitions: `Running ↔ Paused → Finished` (Finished is terminal)
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RunState {
    /// Benchmark is actively running.
    #[default]
    Running,
    /// Benchmark is paused (can resume to Running).
    Paused,
    /// Benchmark has finished (terminal state).
    Finished,
}

/// Pause/resume control shared between the runner and the TUI.
///
/// This is intentionally a small abstraction:
/// - Fast path (`!paused`) is a single atomic load.
/// - Pause wait is event-driven via `Notify`.
///
/// Correctness note: `wait_if_paused()` must subscribe to `Notify` *before*
/// re-checking `paused` to avoid missing a `resume()` notification.
#[derive(Debug, Default)]
pub struct PauseControl {
    paused: AtomicBool,
    resume_notify: tokio::sync::Notify,
}

impl PauseControl {
    /// Create a new pause controller in the running (not paused) state.
    pub fn new() -> Self {
        Self {
            paused: AtomicBool::new(false),
            resume_notify: tokio::sync::Notify::new(),
        }
    }

    /// Returns `true` if the benchmark is currently paused.
    #[inline]
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    /// Pause the benchmark.
    #[inline]
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Release);
    }

    /// Resume the benchmark.
    #[inline]
    pub fn resume(&self) {
        self.paused.store(false, Ordering::Release);
        // Wake all paused workers.
        self.resume_notify.notify_waiters();
    }

    /// Wait until the benchmark is resumed, if it is currently paused.
    #[inline]
    pub async fn wait_if_paused(&self) {
        // Fast path: avoid creating a `notified()` future in the steady-state.
        if !self.is_paused() {
            return;
        }

        loop {
            // Subscribe first to avoid missing a concurrent `resume()`.
            let notified = self.resume_notify.notified();
            if !self.is_paused() {
                return;
            }
            notified.await;
        }
    }
}
