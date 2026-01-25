//! Rolling window statistics for real-time monitoring.
//!
//! This module provides data structures for maintaining sliding window statistics,
//! enabling the calculation of rates and averages over recent time periods.
//!
//! # Overview
//!
//! The rolling window approach divides time into fixed-size buckets. As time progresses,
//! new buckets are added and old ones are dropped, providing a view of recent activity.
//!
//! # Key Types
//!
//! - [`RotateWindow`] - A single rolling window with configurable bucket count.
//! - [`RotateWindowGroup`] - Multiple windows at different time scales (1s, 10s, 1min, 10min).
//! - [`RotateDiffWindowGroup`] - Calculates rate statistics over sliding windows.

use std::{collections::VecDeque, num::NonZeroUsize};

use nonzero_ext::nonzero;
use tokio::time::Duration;

use crate::report::IterReport;

use super::IterStats;

/// A rolling window that maintains statistics in time-ordered buckets.
///
/// The window holds a fixed number of buckets, each containing aggregated
/// statistics for a time period. When rotated, the oldest bucket is dropped
/// and a new empty bucket is added at the front.
///
/// This enables efficient calculation of statistics over the recent past
/// without storing individual data points.
pub struct RotateWindow {
    buckets: VecDeque<IterStats>,
    size: NonZeroUsize,
}

impl RotateWindow {
    fn new(size: NonZeroUsize) -> Self {
        let mut win = Self { buckets: VecDeque::with_capacity(size.get()), size };
        win.rotate(IterStats::new());
        win
    }

    fn push(&mut self, item: &IterReport) {
        // SAFETY: `buckets` is never empty
        *self.buckets.front_mut().unwrap() += item;
    }

    fn rotate(&mut self, bucket: IterStats) {
        if self.buckets.len() == self.size.get() {
            self.buckets.pop_back();
        }
        self.buckets.push_front(bucket);
    }

    fn len(&self) -> usize {
        self.buckets.len()
    }

    fn front(&self) -> &IterStats {
        // SAFETY: `buckets` is never empty
        self.buckets.front().unwrap()
    }

    fn back(&self) -> &IterStats {
        // SAFETY: `buckets` is never empty
        self.buckets.back().unwrap()
    }

    pub fn iter(&self) -> impl Iterator<Item = &IterStats> {
        self.buckets.iter()
    }
}

/// A group of rolling windows at multiple time scales.
///
/// This structure maintains four rolling windows that rotate at different intervals:
/// - `stats_by_sec` - Rotates every second
/// - `stats_by_10sec` - Rotates every 10 seconds
/// - `stats_by_min` - Rotates every minute
/// - `stats_by_10min` - Rotates every 10 minutes
///
/// All windows receive the same data via [`push()`](Self::push), but their buckets
/// represent different time granularities. Call [`rotate()`](Self::rotate) once per
/// second to advance the windows.
pub struct RotateWindowGroup {
    /// Rotation counter (incremented each second).
    pub counter: u64,
    /// Rolling window with 1-second buckets.
    pub stats_by_sec: RotateWindow,
    /// Rolling window with 10-second buckets.
    pub stats_by_10sec: RotateWindow,
    /// Rolling window with 1-minute buckets.
    pub stats_by_min: RotateWindow,
    /// Rolling window with 10-minute buckets.
    pub stats_by_10min: RotateWindow,
}

impl RotateWindowGroup {
    /// Creates a new window group with the specified number of buckets per window.
    ///
    /// # Arguments
    ///
    /// * `buckets` - The number of buckets each window should maintain.
    pub fn new(buckets: NonZeroUsize) -> Self {
        Self {
            counter: 0,
            stats_by_sec: RotateWindow::new(buckets),
            stats_by_10sec: RotateWindow::new(buckets),
            stats_by_min: RotateWindow::new(buckets),
            stats_by_10min: RotateWindow::new(buckets),
        }
    }

    /// Adds an iteration report to all windows.
    ///
    /// The report's statistics are accumulated into the current (front) bucket
    /// of each window.
    pub fn push(&mut self, stats: &IterReport) {
        self.stats_by_sec.push(stats);
        self.stats_by_10sec.push(stats);
        self.stats_by_min.push(stats);
        self.stats_by_10min.push(stats);
    }

    /// Rotates the windows forward by one second.
    ///
    /// This should be called once per second. It creates a new bucket in each
    /// window according to its time scale:
    /// - `stats_by_sec` rotates every call
    /// - `stats_by_10sec` rotates every 10 calls
    /// - `stats_by_min` rotates every 60 calls
    /// - `stats_by_10min` rotates every 600 calls
    pub fn rotate(&mut self) {
        self.counter += 1;
        self.stats_by_sec.rotate(IterStats::new());
        if self.counter.is_multiple_of(10) {
            self.stats_by_10sec.rotate(IterStats::new());
        }
        if self.counter.is_multiple_of(60) {
            self.stats_by_min.rotate(IterStats::new());
        }
        if self.counter.is_multiple_of(600) {
            self.stats_by_10min.rotate(IterStats::new());
        }
    }
}

/// A group of sliding windows for calculating rate statistics.
///
/// Unlike [`RotateWindowGroup`] which stores raw statistics, this structure
/// stores cumulative snapshots and calculates differences to determine rates
/// over various time periods.
///
/// The windows rotate at a configurable frame rate (fps), storing snapshots
/// of cumulative statistics. Rate calculations compare the newest and oldest
/// snapshots to compute activity over the window's time span.
///
/// # Time Windows
///
/// - Last 1 second
/// - Last 10 seconds
/// - Last 1 minute
/// - Last 10 minutes
pub struct RotateDiffWindowGroup {
    interval: Duration,
    stats_last_sec: RotateWindow,
    stats_last_10sec: RotateWindow,
    stats_last_min: RotateWindow,
    stats_last_10min: RotateWindow,
}

impl RotateDiffWindowGroup {
    /// Returns mutable references to all internal windows.
    fn all_stats(&mut self) -> [&mut RotateWindow; 4] {
        [
            &mut self.stats_last_sec,
            &mut self.stats_last_10sec,
            &mut self.stats_last_min,
            &mut self.stats_last_10min,
        ]
    }

    /// Creates a new diff window group with the specified frame rate.
    ///
    /// The frame rate determines how often [`rotate()`](Self::rotate) should be called.
    /// Higher frame rates provide smoother statistics but use more memory.
    ///
    /// # Arguments
    ///
    /// * `fps` - Frames (rotations) per second.
    pub fn new(fps: NonZeroUsize) -> Self {
        let interval = Duration::from_secs_f64(1.0 / fps.get() as f64);
        let mut group = Self {
            interval,
            stats_last_sec: RotateWindow::new(fps.saturating_add(1)),
            stats_last_10sec: RotateWindow::new(fps.saturating_mul(nonzero!(10usize)).saturating_add(1)),
            stats_last_min: RotateWindow::new(fps.saturating_mul(nonzero!(60usize)).saturating_add(1)),
            stats_last_10min: RotateWindow::new(fps.saturating_mul(nonzero!(600usize)).saturating_add(1)),
        };
        group.rotate(&IterStats::new());
        group
    }

    /// Rotates all windows with a new cumulative statistics snapshot.
    ///
    /// This should be called at the configured frame rate (fps times per second).
    ///
    /// # Arguments
    ///
    /// * `stats` - The current cumulative statistics snapshot.
    pub fn rotate(&mut self, stats: &IterStats) {
        for s in self.all_stats().iter_mut() {
            s.rotate(stats.clone());
        }
    }

    /// Returns statistics delta and duration for the last 1 second.
    pub fn stats_last_sec(&self) -> (IterStats, Duration) {
        self.diff(&self.stats_last_sec)
    }

    /// Returns statistics delta and duration for the last 10 seconds.
    pub fn stats_last_10sec(&self) -> (IterStats, Duration) {
        self.diff(&self.stats_last_10sec)
    }

    /// Returns statistics delta and duration for the last 1 minute.
    pub fn stats_last_min(&self) -> (IterStats, Duration) {
        self.diff(&self.stats_last_min)
    }

    /// Returns statistics delta and duration for the last 10 minutes.
    pub fn stats_last_10min(&self) -> (IterStats, Duration) {
        self.diff(&self.stats_last_10min)
    }

    /// Calculates the difference between the newest and oldest snapshots.
    fn diff(&self, win: &RotateWindow) -> (IterStats, Duration) {
        let duration = (win.len() - 1) as u32 * self.interval;
        (win.front() - win.back(), duration)
    }
}
