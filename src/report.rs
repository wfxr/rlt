//! The benchmark report module.
use std::collections::HashMap;

use tokio::time::Duration;

use crate::{
    histogram::LatencyHistogram,
    stats::IterStats,
    status::{Status, StatusKind},
};

/// The iteration report.
#[derive(Debug, Clone)]
pub struct IterReport {
    /// The reported duration of the iteration.
    pub duration: Duration,
    /// The reported status of the iteration.
    pub status: Status,
    /// The reported processed bytes of the iteration.
    pub bytes: u64,
    /// The reported processed items of the iteration. Useful when testing services with batch support.
    pub items: u64,
}

/// The final benchmark report.
pub struct BenchReport {
    /// Number of workers to run concurrently
    pub concurrency: u32,
    /// Iteration latency histogram.
    pub hist: LatencyHistogram,
    /// Iteration statistics.
    pub stats: IterStats,
    /// Status distribution.
    pub status_dist: HashMap<Status, u64>,
    /// Error distribution.
    pub error_dist: HashMap<String, u64>,
    /// The total elapsed time of the benchmark.
    pub elapsed: Duration,
}

impl BenchReport {
    /// Returns the success ratio of the benchmark.
    pub fn success_ratio(&self) -> f64 {
        if self.stats.overall.iters == 0 {
            return 0.0;
        }
        self.stats
            .by_status
            .iter()
            .filter(|(k, _)| k.kind() == StatusKind::Success)
            .map(|(_, v)| v.iters as f64)
            .sum::<f64>()
            / self.stats.overall.iters as f64
    }
}
