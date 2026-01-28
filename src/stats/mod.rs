//! Statistics tracking for benchmark iterations.
//!
//! This module provides data structures for collecting and aggregating benchmark
//! statistics across iterations. It supports both cumulative totals and rolling
//! window statistics for real-time monitoring.
//!
//! # Key Types
//!
//! - [`Counter`] - Tracks basic metrics: iterations, items, bytes, and duration.
//! - [`IterStats`] - Aggregates iteration statistics with per-status breakdowns.
//! - [`RotateWindowGroup`] - Manages multiple rolling windows at different time scales.
//! - [`RotateDiffWindow`] - Provides rate calculations over sliding windows.

mod counter;
mod window;

pub use counter::Counter;
pub use window::{RotateDiffWindow, RotateWindowGroup};

use std::collections::HashMap;

use crate::{report::IterReport, status::Status};

/// Aggregated statistics for benchmark iterations.
///
/// This structure collects statistics across multiple iterations, providing both
/// a cumulative total and per-status breakdowns. It's used internally by the
/// benchmark framework to track progress and generate reports.
///
/// # Fields
///
/// - `counter` - Cumulative totals across all iterations.
/// - `details` - Per-status breakdown of counters (e.g., separate counts for
///   successful vs failed iterations).
#[derive(Clone, Debug)]
pub struct IterStats {
    /// Cumulative totals across all iterations.
    pub counter: Counter,
    /// Per-status breakdown of counters.
    pub details: HashMap<Status, Counter>,
}

impl IterStats {
    pub fn new() -> Self {
        Self { counter: Counter::default(), details: HashMap::new() }
    }

    pub fn append(&mut self, stats: &IterReport) {
        self.counter.append(stats);
        let counter = self.details.entry(stats.status).or_default();
        counter.append(stats);
    }
}

impl Default for IterStats {
    fn default() -> Self {
        Self::new()
    }
}
