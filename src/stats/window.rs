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
//! - [`StatsWindow`] - A single rolling window with configurable bucket count.
//! - [`MultiScaleStatsWindow`] - Multiple windows at different time scales.
//! - [`RecentStatsWindow`] - Calculates rate statistics over sliding windows.

use std::collections::VecDeque;
use std::num::NonZeroUsize;

use itertools::Itertools;
use nonzero_ext::nonzero;
use tokio::time::Duration;

use super::Counter;
use crate::error::ConfigError;
use crate::report::IterReport;

/// A rolling window that maintains statistics in time-ordered buckets.
///
/// The window holds a fixed number of buckets, each containing aggregated
/// statistics for a time period. When rotated, the oldest bucket is dropped
/// and a new empty bucket is added at the front.
///
/// This enables efficient calculation of statistics over the recent past
/// without storing individual data points.
pub struct StatsWindow {
    buckets: VecDeque<Counter>,
    size: NonZeroUsize,
}

impl StatsWindow {
    fn new(size: NonZeroUsize) -> Self {
        let mut win = Self { buckets: VecDeque::with_capacity(size.get()), size };
        win.rotate(Counter::default());
        win
    }

    fn push(&mut self, item: &IterReport) {
        // SAFETY: `buckets` is never empty
        self.buckets.front_mut().unwrap().record(item);
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

/// A multi-scale stats window backed by multiple rolling windows.
///
/// This structure maintains multiple rolling windows that rotate at different
/// intervals configured by the caller.
///
/// All windows receive the same data via [`push()`](Self::push), but their buckets
/// represent different time granularities. Call [`tick()`](Self::tick) once per
/// second to advance the windows.
pub struct MultiScaleStatsWindow {
    /// Seconds tick counter (incremented once per second).
    pub tick: u64,
    periods: Vec<usize>,
    windows: Vec<StatsWindow>,
}

impl MultiScaleStatsWindow {
    /// Creates a new multi-scale stats window with the specified number of buckets per window.
    ///
    /// # Arguments
    ///
    /// * `buckets` - The number of buckets each window should maintain.
    /// * `periods` - Rotation periods in seconds for each window.
    ///
    /// # Errors
    ///
    /// Returns an error if `periods` is empty or contains zero.
    pub fn new<I, P>(buckets: NonZeroUsize, periods: I) -> std::result::Result<Self, ConfigError>
    where
        I: IntoIterator<Item = P>,
        P: Into<usize>,
    {
        let periods = periods.into_iter().map(Into::into).collect_vec();
        if periods.is_empty() {
            return Err(ConfigError::WindowPeriodsEmpty);
        }
        if let Some(&period) = periods.iter().find(|p| **p == 0) {
            return Err(ConfigError::WindowPeriodZero { period });
        }
        let windows = periods.iter().map(|_| StatsWindow::new(buckets)).collect();
        Ok(Self { tick: 0, periods, windows })
    }

    /// Adds an iteration report to all windows.
    ///
    /// The report's statistics are accumulated into the current (front) bucket
    /// of each window.
    pub fn push(&mut self, stats: &IterReport) {
        for win in &mut self.windows {
            win.push(stats);
        }
    }

    /// Advances the windows forward by one second.
    ///
    /// Call this once per second. It creates a new bucket in each window according to its
    /// configured period.
    pub fn tick(&mut self) {
        self.tick += 1;
        for (period, win) in self.periods.iter().zip(self.windows.iter_mut()) {
            if self.tick % (*period as u64) == 0 {
                win.rotate(Counter::default());
            }
        }
    }

    /// Returns the window matching the requested period in seconds.
    pub fn window_for_secs(&self, secs: usize) -> Option<&StatsWindow> {
        self.periods.iter().position(|p| *p == secs).map(|idx| &self.windows[idx])
    }
}

/// A sliding window for calculating rate statistics over arbitrary time spans.
///
/// Unlike [`MultiScaleStatsWindow`] which stores raw statistics in separate windows,
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
/// let mut win = RecentStatsWindow::new(nonzero!(10usize)); // 10 fps
/// win.record(counter);
/// let (delta, duration) = win.stats_for_secs(1);  // last 1 second
/// let (delta, duration) = win.stats_for_secs(60); // last 1 minute
/// ```
pub struct RecentStatsWindow {
    interval: Duration,
    fps: usize,
    window: StatsWindow,
}

impl RecentStatsWindow {
    /// Creates a new recent stats window with the specified frame rate.
    ///
    /// The frame rate determines how often [`record()`](Self::record) should be called.
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
            window: StatsWindow::new(fps.saturating_mul(nonzero!(600usize)).saturating_add(1)),
        };
        win.record(Counter::default());
        win
    }

    /// Records a new cumulative statistics snapshot.
    ///
    /// This should be called at the configured frame rate (fps times per second).
    ///
    /// # Arguments
    ///
    /// * `total` - The current cumulative statistics snapshot.
    pub fn record(&mut self, total: Counter) {
        self.window.rotate(total);
    }

    /// Returns statistics delta and duration for the specified time window.
    ///
    /// If the window doesn't have enough data for the requested time span,
    /// returns the maximum available data and the actual duration covered.
    ///
    /// # Arguments
    ///
    /// * `secs` - The time window in seconds (e.g., 1, 10, 60, 600).
    pub fn stats_for_secs(&self, secs: usize) -> (Counter, Duration) {
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
            latency_sum: Duration::from_millis(iters),
        }
    }

    #[test]
    fn recent_stats_window_one_sec() {
        let fps = nonzero!(10usize); // 10 frames per second
        let mut win = RecentStatsWindow::new(fps);

        // Simulate 1 second of data (10 frames at 10 fps)
        for i in 1..=10 {
            win.record(make_counter(i * 100));
        }

        let (counter, duration) = win.stats_for_secs(1);
        // Diff between frame 10 (iters=1000) and frame 0 (iters=0)
        assert_eq!(counter.iters, 1000);
        assert_eq!(duration, Duration::from_secs(1));
    }

    #[test]
    fn recent_stats_window_partial_fill() {
        let fps = nonzero!(10usize);
        let mut win = RecentStatsWindow::new(fps);

        // Only 5 frames (0.5 second worth)
        for i in 1..=5 {
            win.record(make_counter(i * 10));
        }

        let (counter, duration) = win.stats_for_secs(1);
        // Window not full yet, should use available data
        // Note: new() initializes with 2 default counters (one in StatsWindow::new, one via `record`)
        // So after 5 rotations we have 7 elements total, giving (7-1) * 100ms = 600ms
        assert_eq!(counter.iters, 50); // 50 - 0
        assert_eq!(duration, Duration::from_millis(600)); // 6 intervals * 100ms
    }

    #[test]
    fn recent_stats_window_multiple_spans() {
        let fps = nonzero!(10usize);
        let mut win = RecentStatsWindow::new(fps);

        // Fill enough for 10 seconds (100 frames)
        for i in 1..=100 {
            win.record(make_counter(i));
        }

        let (c1, d1) = win.stats_for_secs(1);
        let (c10, d10) = win.stats_for_secs(10);

        // 1 sec = 10 frames: diff between frame 100 and frame 90
        assert_eq!(c1.iters, 100 - 90);
        assert_eq!(d1, Duration::from_secs(1));

        // 10 sec = 100 frames: diff between frame 100 and frame 0
        assert_eq!(c10.iters, 100);
        assert_eq!(d10, Duration::from_secs(10));
    }

    #[test]
    fn multi_scale_stats_window_ticks_by_periods() {
        let mut msw =
            MultiScaleStatsWindow::new(nonzero!(2usize), [1usize, 10]).expect("valid periods");
        assert_eq!(msw.window_for_secs(10).unwrap().len(), 1);

        for _ in 0..9 {
            msw.tick();
        }
        assert_eq!(msw.window_for_secs(10).unwrap().len(), 1);

        msw.tick();
        assert_eq!(msw.window_for_secs(10).unwrap().len(), 2);
        assert_eq!(msw.window_for_secs(1).unwrap().len(), 2);
    }

    #[test]
    fn multi_scale_stats_window_rejects_zero_period() {
        let err =
            MultiScaleStatsWindow::new(nonzero!(2usize), [0usize]).err().expect("expected error");
        assert_eq!(err.to_string(), "stats window period must be > 0 (got 0)");
    }

    #[test]
    fn multi_scale_stats_window_rejects_empty_periods() {
        let err = MultiScaleStatsWindow::new(nonzero!(2usize), std::iter::empty::<usize>())
            .err()
            .expect("expected error");
        assert_eq!(err.to_string(), "stats window periods must be non-empty");
    }
}
