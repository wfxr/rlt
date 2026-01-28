//! Basic counter for tracking benchmark metrics.
//!
//! This module provides [`Counter`], a simple structure for accumulating
//! iteration counts, item counts, byte counts, and total latency.

use std::time::Duration;

use crate::report::IterReport;

/// A counter for accumulating benchmark metrics.
///
/// This structure tracks the fundamental metrics of a benchmark run:
/// - Number of iterations completed
/// - Number of items processed (e.g., requests, records)
/// - Number of bytes transferred
/// - Total latency of all iterations
///
/// # Operations
///
/// - Can be accumulated from [`IterReport`] using [`record`](Self::record).
/// - Supports subtraction via `-=` for calculating deltas.
///
/// # Example
///
/// ```ignore
/// let mut counter = Counter::default();
/// counter.record(&iter_report);  // Accumulate from an iteration report
/// println!("Total iterations: {}", counter.iters);
/// ```
#[derive(Default, Clone, Copy, Debug)]
pub struct Counter {
    /// Number of iterations completed.
    pub iters: u64,
    /// Number of items processed (e.g., requests, records, operations).
    pub items: u64,
    /// Number of bytes transferred or processed.
    pub bytes: u64,
    /// Sum of iteration latencies.
    pub latency_sum: Duration,
}

impl Counter {
    /// Accumulate metrics from a single iteration report.
    pub fn record(&mut self, report: &IterReport) {
        self.iters += 1;
        self.items += report.items;
        self.bytes += report.bytes;
        self.latency_sum += report.duration;
    }
}

impl std::ops::SubAssign<&Counter> for Counter {
    fn sub_assign(&mut self, rhs: &Counter) {
        self.iters -= rhs.iters;
        self.items -= rhs.items;
        self.bytes -= rhs.bytes;
        self.latency_sum -= rhs.latency_sum;
    }
}

impl std::ops::Sub<&Counter> for &Counter {
    type Output = Counter;

    fn sub(self, rhs: &Counter) -> Counter {
        let mut out = *self;
        out -= rhs;
        out
    }
}
