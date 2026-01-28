//! Statistics tracking for benchmark iterations.
//!
//! This module provides data structures for collecting and aggregating benchmark
//! statistics across iterations. It supports both cumulative totals and rolling
//! window statistics for real-time monitoring.
//!
//! # Key Types
//!
//! - [`Counter`] - Tracks basic metrics: iterations, items, bytes, and latency sum.
//! - [`IterStats`] - Aggregates iteration statistics with per-status breakdowns.
//! - [`MultiScaleStatsWindow`] - Manages multiple rolling windows at configurable time scales.
//! - [`RecentStatsWindow`] - Provides rate calculations over sliding windows.

mod counter;
mod window;

pub use counter::Counter;
pub use window::{MultiScaleStatsWindow, RecentStatsWindow};

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
/// - `overall` - Cumulative totals across all iterations.
/// - `by_status` - Per-status breakdown of counters (e.g., separate counts for
///   successful vs failed iterations).
#[derive(Clone, Debug)]
pub struct IterStats {
    /// Cumulative totals across all iterations.
    pub overall: Counter,
    /// Per-status breakdown of counters.
    pub by_status: HashMap<Status, Counter>,
}

impl IterStats {
    pub fn new() -> Self {
        Self { overall: Counter::default(), by_status: HashMap::new() }
    }

    pub fn record(&mut self, report: &IterReport) {
        self.overall.record(report);
        let counter = self.by_status.entry(report.status).or_default();
        counter.record(report);
    }
}

impl Default for IterStats {
    fn default() -> Self {
        Self::new()
    }
}
