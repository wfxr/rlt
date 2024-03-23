use std::collections::HashMap;

use tokio::time::Duration;

use crate::{
    histogram::LatencyHistogram,
    stats::IterStats,
    status::{Status, StatusKind},
};

#[derive(Debug, Clone)]
pub struct IterReport {
    pub duration: Duration,
    pub status:   Status,
    pub bytes:    u64,
    pub items:    u64,
}

pub struct BenchReport {
    pub concurrency: u32,
    pub hist:        LatencyHistogram,
    pub stats:       IterStats,
    pub status_dist: HashMap<Status, u64>,
    pub error_dist:  HashMap<String, u64>,
    pub elapsed:     Duration,
}

impl BenchReport {
    pub fn success_ratio(&self) -> f64 {
        if self.stats.counter.iters == 0 {
            return 0.0;
        }
        self.stats
            .details
            .iter()
            .filter(|(k, _)| k.kind() == StatusKind::Success)
            .map(|(_, v)| v.iters as f64)
            .sum::<f64>()
            / self.stats.counter.iters as f64
    }
}
