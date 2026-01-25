//! Basic counter for tracking benchmark metrics.
//!
//! This module provides [`Counter`], a simple structure for accumulating
//! iteration counts, item counts, byte counts, and total duration.

use std::time::Duration;

use crate::report::IterReport;

/// A counter for accumulating benchmark metrics.
///
/// This structure tracks the fundamental metrics of a benchmark run:
/// - Number of iterations completed
/// - Number of items processed (e.g., requests, records)
/// - Number of bytes transferred
/// - Total duration of all iterations
///
/// # Operations
///
/// - Can be accumulated from [`IterReport`] using `+=` operator.
/// - Supports subtraction via `-=` for calculating deltas.
///
/// # Example
///
/// ```ignore
/// let mut counter = Counter::default();
/// counter += &iter_report;  // Accumulate from an iteration report
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
    /// Total duration of all iterations combined.
    pub duration: Duration,
}

impl std::ops::AddAssign<&IterReport> for Counter {
    fn add_assign(&mut self, stats: &IterReport) {
        self.iters += 1;
        self.items += stats.items;
        self.bytes += stats.bytes;
        self.duration += stats.duration;
    }
}

impl std::ops::SubAssign<&Counter> for Counter {
    fn sub_assign(&mut self, rhs: &Counter) {
        self.iters -= rhs.iters;
        self.items -= rhs.items;
        self.bytes -= rhs.bytes;
        self.duration -= rhs.duration;
    }
}
