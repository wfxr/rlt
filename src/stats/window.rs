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
//! - [`RotateDiffWindow`] - Calculates rate statistics over sliding windows.

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

    fn get(&self, index: usize) -> Option<&Counter> {
        self.buckets.get(index)
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

/// A sliding window for calculating rate statistics over arbitrary time spans.
///
/// Unlike [`RotateWindowGroup`] which stores raw statistics in separate windows,
/// this structure stores cumulative snapshots in a single window and calculates
/// differences to determine rates over any requested time period.
///
/// The window rotates at a configurable frame rate (fps), storing snapshots
/// of cumulative statistics. Rate calculations compare the current snapshot
/// with a snapshot from N frames ago to compute activity over the time span.
///
/// Note: This stores only [`Counter`] snapshots (not `IterStats`) to keep the per-frame
/// rotation in the TUI hot path allocation-free.
///
/// # Example
///
/// ```ignore
/// let mut win = RotateDiffWindow::new(nonzero!(10usize)); // 10 fps
/// win.rotate(counter);
/// let (delta, duration) = win.counter_for_secs(1);  // last 1 second
/// let (delta, duration) = win.counter_for_secs(60); // last 1 minute
/// ```
pub struct RotateDiffWindow {
    interval: Duration,
    fps: usize,
    window: RotateWindow,
}

impl RotateDiffWindow {
    /// Creates a new diff window with the specified frame rate.
    ///
    /// The frame rate determines how often [`rotate()`](Self::rotate) should be called.
    /// Higher frame rates provide smoother statistics but use more memory.
    ///
    /// # Arguments
    ///
    /// * `fps` - Frames (rotations) per second.
    pub fn new(fps: NonZeroUsize) -> Self {
        let interval = Duration::from_secs_f64(1.0 / fps.get() as f64);
        let mut win = Self {
            interval,
            fps: fps.get(),
            window: RotateWindow::new(fps.saturating_mul(nonzero!(600usize)).saturating_add(1)),
        };
        win.rotate(Counter::default());
        win
    }

    /// Rotates the window with a new cumulative statistics snapshot.
    ///
    /// This should be called at the configured frame rate (fps times per second).
    ///
    /// # Arguments
    ///
    /// * `counter` - The current cumulative statistics snapshot.
    pub fn rotate(&mut self, counter: Counter) {
        self.window.rotate(counter);
    }

    /// Returns statistics delta and duration for the specified time window.
    ///
    /// If the window doesn't have enough data for the requested time span,
    /// returns the maximum available data and the actual duration covered.
    ///
    /// # Arguments
    ///
    /// * `secs` - The time window in seconds (e.g., 1, 10, 60, 600).
    pub fn counter_for_secs(&self, secs: usize) -> (Counter, Duration) {
        let frames_back = self.fps * secs;
        let clamped = frames_back.min(self.window.len().saturating_sub(1));
        let duration = clamped as u32 * self.interval;
        let back = self.window.get(clamped).unwrap_or_else(|| self.window.back());
        (self.window.front() - back, duration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_counter(iters: u64) -> Counter {
        Counter {
            iters,
            items: iters * 10,
            bytes: iters * 100,
            duration: Duration::from_millis(iters),
        }
    }

    #[test]
    fn rotate_diff_window_one_sec() {
        let fps = nonzero!(10usize); // 10 frames per second
        let mut win = RotateDiffWindow::new(fps);

        // Simulate 1 second of data (10 frames at 10 fps)
        for i in 1..=10 {
            win.rotate(make_counter(i * 100));
        }

        let (counter, duration) = win.counter_for_secs(1);
        // Diff between frame 10 (iters=1000) and frame 0 (iters=0)
        assert_eq!(counter.iters, 1000);
        assert_eq!(duration, Duration::from_secs(1));
    }

    #[test]
    fn rotate_diff_window_partial_fill() {
        let fps = nonzero!(10usize);
        let mut win = RotateDiffWindow::new(fps);

        // Only 5 frames (0.5 second worth)
        for i in 1..=5 {
            win.rotate(make_counter(i * 10));
        }

        let (counter, duration) = win.counter_for_secs(1);
        // Window not full yet, should use available data
        // Note: new() initializes with 2 default counters (one in RotateWindow::new, one in win.rotate)
        // So after 5 rotations we have 7 elements total, giving (7-1) * 100ms = 600ms
        assert_eq!(counter.iters, 50); // 50 - 0
        assert_eq!(duration, Duration::from_millis(600)); // 6 intervals * 100ms
    }

    #[test]
    fn rotate_diff_window_multiple_spans() {
        let fps = nonzero!(10usize);
        let mut win = RotateDiffWindow::new(fps);

        // Fill enough for 10 seconds (100 frames)
        for i in 1..=100 {
            win.rotate(make_counter(i));
        }

        let (c1, d1) = win.counter_for_secs(1);
        let (c10, d10) = win.counter_for_secs(10);

        // 1 sec = 10 frames: diff between frame 100 and frame 90
        assert_eq!(c1.iters, 100 - 90);
        assert_eq!(d1, Duration::from_secs(1));

        // 10 sec = 100 frames: diff between frame 100 and frame 0
        assert_eq!(c10.iters, 100 - 0);
        assert_eq!(d10, Duration::from_secs(10));
    }
}
