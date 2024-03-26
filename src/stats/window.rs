use std::{
    collections::VecDeque,
    iter::{once, repeat_with},
};

use tokio::time::{Duration, Instant};

use crate::report::IterReport;

use super::IterStats;

pub struct RotateWindow {
    buckets: VecDeque<IterStats>,
    size: usize,
}

impl RotateWindow {
    fn new(size: usize) -> Self {
        assert!(size > 0);
        let mut win = Self { buckets: VecDeque::with_capacity(size), size };
        win.rotate(IterStats::new());
        win
    }

    fn push(&mut self, item: &IterReport) {
        // SAFETY: `buckets` is never empty
        *self.buckets.front_mut().unwrap() += item;
    }

    fn rotate(&mut self, bucket: IterStats) {
        if self.buckets.len() == self.size {
            self.buckets.pop_back();
        }
        self.buckets.push_front(bucket);
    }

    fn rotate_multi(&mut self, buckets: impl Iterator<Item = IterStats>) {
        buckets.for_each(|bucket| self.rotate(bucket));
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
    frame: Instant,
    pub stats_by_sec: RotateWindow,
    pub stats_by_10sec: RotateWindow,
    pub stats_by_min: RotateWindow,
    pub stats_by_10min: RotateWindow,
}

impl RotateWindowGroup {
    pub fn new(frame: Instant, buckets: usize) -> Self {
        Self {
            frame,
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

    pub fn rotate(&mut self, now: Instant) {
        let duration = now - self.frame;
        if duration.as_secs() == 0 {
            return;
        }
        self.stats_by_sec.rotate(IterStats::new());
        if duration.as_secs() % 10 == 0 {
            self.stats_by_10sec.rotate(IterStats::new());
        }
        if duration.as_secs() % 60 == 0 {
            self.stats_by_min.rotate(IterStats::new());
        }
        if duration.as_secs() % 600 == 0 {
            self.stats_by_10min.rotate(IterStats::new());
        }
    }
}

pub struct RotateDiffWindowGroup {
    frame: Instant,
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
    pub fn new(frame: Instant, fps: u8) -> Self {
        let fps = fps as usize;
        let interval = Duration::from_secs_f64(1.0 / fps as f64);
        let frame = frame - interval;
        let mut group = Self {
            frame,
            interval,
            stats_last_sec: RotateWindow::new(fps + 1),
            stats_last_10sec: RotateWindow::new(fps * 10 + 1),
            stats_last_min: RotateWindow::new(fps * 60 + 1),
            stats_last_10min: RotateWindow::new(fps * 600 + 1),
        };
        group.rotate(frame, &IterStats::new());
        group
    }

    pub fn rotate(&mut self, next_frame: Instant, stats: &IterStats) {
        if next_frame < self.frame + self.interval {
            return;
        }

        let duration = next_frame - self.frame;
        let frames = (duration.as_millis() / self.interval.as_millis()) as usize;
        let buckets = repeat_with(IterStats::new).take(frames - 1).chain(once(stats.clone()));
        for s in self.all_stats().iter_mut() {
            s.rotate_multi(buckets.clone());
        }
        self.frame += self.interval * frames as u32;
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
