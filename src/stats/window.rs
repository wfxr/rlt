use std::{collections::VecDeque, num::NonZeroUsize};

use nonzero_ext::nonzero;
use tokio::time::Duration;

use crate::report::IterReport;

use super::IterStats;

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

pub struct RotateWindowGroup {
    pub counter: u64,
    pub stats_by_sec: RotateWindow,
    pub stats_by_10sec: RotateWindow,
    pub stats_by_min: RotateWindow,
    pub stats_by_10min: RotateWindow,
}

impl RotateWindowGroup {
    pub fn new(buckets: NonZeroUsize) -> Self {
        Self {
            counter: 0,
            stats_by_sec: RotateWindow::new(buckets),
            stats_by_10sec: RotateWindow::new(buckets),
            stats_by_min: RotateWindow::new(buckets),
            stats_by_10min: RotateWindow::new(buckets),
        }
    }

    pub fn push(&mut self, stats: &IterReport) {
        self.stats_by_sec.push(stats);
        self.stats_by_10sec.push(stats);
        self.stats_by_min.push(stats);
        self.stats_by_10min.push(stats);
    }

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

pub struct RotateDiffWindowGroup {
    interval: Duration,
    stats_last_sec: RotateWindow,
    stats_last_10sec: RotateWindow,
    stats_last_min: RotateWindow,
    stats_last_10min: RotateWindow,
}

impl RotateDiffWindowGroup {
    fn all_stats(&mut self) -> [&mut RotateWindow; 4] {
        [
            &mut self.stats_last_sec,
            &mut self.stats_last_10sec,
            &mut self.stats_last_min,
            &mut self.stats_last_10min,
        ]
    }
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

    pub fn rotate(&mut self, stats: &IterStats) {
        for s in self.all_stats().iter_mut() {
            s.rotate(stats.clone());
        }
    }

    pub fn stats_last_sec(&self) -> (IterStats, Duration) {
        self.diff(&self.stats_last_sec)
    }

    pub fn stats_last_10sec(&self) -> (IterStats, Duration) {
        self.diff(&self.stats_last_10sec)
    }

    pub fn stats_last_min(&self) -> (IterStats, Duration) {
        self.diff(&self.stats_last_min)
    }

    pub fn stats_last_10min(&self) -> (IterStats, Duration) {
        self.diff(&self.stats_last_10min)
    }

    fn diff(&self, win: &RotateWindow) -> (IterStats, Duration) {
        let duration = (win.len() - 1) as u32 * self.interval;
        (win.front() - win.back(), duration)
    }
}
