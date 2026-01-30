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
