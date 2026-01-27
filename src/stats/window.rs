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

use super::Counter;

/// A rolling window that maintains statistics in time-ordered buckets.
///
/// The window holds a fixed number of buckets, each containing aggregated
/// statistics for a time period. When rotated, the oldest bucket is dropped
/// and a new empty bucket is added at the front.
///
/// This enables efficient calculation of statistics over the recent past
/// without storing individual data points.
pub struct RotateWindow {
    buckets: VecDeque<Counter>,
    size: NonZeroUsize,
}

impl RotateWindow {
    fn new(size: NonZeroUsize) -> Self {
        let mut win = Self { buckets: VecDeque::with_capacity(size.get()), size };
        win.rotate(Counter::default());
        win
    }

    fn push(&mut self, item: &IterReport) {
        // SAFETY: `buckets` is never empty
        self.buckets.front_mut().unwrap().append(item);
    }

    fn rotate(&mut self, bucket: Counter) {
        if self.buckets.len() == self.size.get() {
            self.buckets.pop_back();
        }
        self.buckets.push_front(bucket);
    }

    fn len(&self) -> usize {
        self.buckets.len()
    }

    fn front(&self) -> &Counter {
        // SAFETY: `buckets` is never empty
        self.buckets.front().unwrap()
    }

    fn back(&self) -> &Counter {
        // SAFETY: `buckets` is never empty
        self.buckets.back().unwrap()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Counter> {
        self.buckets.iter()
    }
}

/// A group of rolling windows at multiple time scales.
///
/// This structure maintains four rolling windows that rotate at different intervals:
/// - `counters_by_sec` - Rotates every second
/// - `counters_by_10sec` - Rotates every 10 seconds
/// - `counters_by_min` - Rotates every minute
/// - `counters_by_10min` - Rotates every 10 minutes
///
/// All windows receive the same data via [`push()`](Self::push), but their buckets
/// represent different time granularities. Call [`rotate()`](Self::rotate) once per
/// second to advance the windows.
pub struct RotateWindowGroup {
    /// Rotation counter (incremented each second).
    pub counter: u64,
    /// Rolling window with 1-second buckets.
    pub counters_by_sec: RotateWindow,
    /// Rolling window with 10-second buckets.
    pub counters_by_10sec: RotateWindow,
    /// Rolling window with 1-minute buckets.
    pub counters_by_min: RotateWindow,
    /// Rolling window with 10-minute buckets.
    pub counters_by_10min: RotateWindow,
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
            counters_by_sec: RotateWindow::new(buckets),
            counters_by_10sec: RotateWindow::new(buckets),
            counters_by_min: RotateWindow::new(buckets),
            counters_by_10min: RotateWindow::new(buckets),
        }
    }

    /// Adds an iteration report to all windows.
    ///
    /// The report's statistics are accumulated into the current (front) bucket
    /// of each window.
    pub fn push(&mut self, stats: &IterReport) {
        self.counters_by_sec.push(stats);
        self.counters_by_10sec.push(stats);
        self.counters_by_min.push(stats);
        self.counters_by_10min.push(stats);
    }

    /// Rotates the windows forward by one second.
    ///
    /// This should be called once per second. It creates a new bucket in each
    /// window according to its time scale:
    /// - `counters_by_sec` rotates every call
    /// - `counters_by_10sec` rotates every 10 calls
    /// - `counters_by_min` rotates every 60 calls
    /// - `counters_by_10min` rotates every 600 calls
    pub fn rotate(&mut self) {
        self.counter += 1;
        self.counters_by_sec.rotate(Counter::default());
        if self.counter % 10 == 0 {
            self.counters_by_10sec.rotate(Counter::default());
        }
        if self.counter % 60 == 0 {
            self.counters_by_min.rotate(Counter::default());
        }
        if self.counter % 600 == 0 {
            self.counters_by_10min.rotate(Counter::default());
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
/// Note: This stores only [`Counter`] snapshots (not `IterStats`) to keep the per-frame
/// rotation in the TUI hot path allocation-free.
///
/// # Time Windows
///
/// - Last 1 second
/// - Last 10 seconds
/// - Last 1 minute
/// - Last 10 minutes
pub struct RotateDiffWindowGroup {
    interval: Duration,
    counters_last_sec: RotateWindow,
    counters_last_10sec: RotateWindow,
    counters_last_min: RotateWindow,
    counters_last_10min: RotateWindow,
}

impl RotateDiffWindowGroup {
    /// Returns mutable references to all internal windows.
    fn all_windows(&mut self) -> [&mut RotateWindow; 4] {
        [
            &mut self.counters_last_sec,
            &mut self.counters_last_10sec,
            &mut self.counters_last_min,
            &mut self.counters_last_10min,
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
            counters_last_sec: RotateWindow::new(fps.saturating_add(1)),
            counters_last_10sec: RotateWindow::new(fps.saturating_mul(nonzero!(10usize)).saturating_add(1)),
            counters_last_min: RotateWindow::new(fps.saturating_mul(nonzero!(60usize)).saturating_add(1)),
            counters_last_10min: RotateWindow::new(fps.saturating_mul(nonzero!(600usize)).saturating_add(1)),
        };
        group.rotate(Counter::default());
        group
    }

    /// Rotates all windows with a new cumulative statistics snapshot.
    ///
    /// This should be called at the configured frame rate (fps times per second).
    ///
    /// # Arguments
    ///
    /// * `counter` - The current cumulative statistics snapshot.
    pub fn rotate(&mut self, counter: Counter) {
        // Hot path (called at FPS): rotate cheap `Counter` snapshots to avoid per-frame allocations.
        for s in self.all_windows().iter_mut() {
            s.rotate(counter);
        }
    }

    /// Returns statistics delta and duration for the last 1 second.
    pub fn counter_last_sec(&self) -> (Counter, Duration) {
        self.diff(&self.counters_last_sec)
    }

    /// Returns statistics delta and duration for the last 10 seconds.
    pub fn counter_last_10sec(&self) -> (Counter, Duration) {
        self.diff(&self.counters_last_10sec)
    }

    /// Returns statistics delta and duration for the last 1 minute.
    pub fn counter_last_min(&self) -> (Counter, Duration) {
        self.diff(&self.counters_last_min)
    }

    /// Returns statistics delta and duration for the last 10 minutes.
    pub fn counter_last_10min(&self) -> (Counter, Duration) {
        self.diff(&self.counters_last_10min)
    }

    /// Calculates the difference between the newest and oldest snapshots.
    fn diff(&self, win: &RotateWindow) -> (Counter, Duration) {
        let duration = (win.len() - 1) as u32 * self.interval;
        (win.front() - win.back(), duration)
    }
}
