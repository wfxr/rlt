//! Silent (headless) report collector.
//!
//! This module provides [`SilentCollector`], a non-interactive collector
//! that aggregates benchmark results without any terminal output.
//!
//! # Use Cases
//!
//! - CI/CD pipelines where interactive TUI is not available
//! - Scripted benchmarks where only the final report matters
//! - Environments without terminal capabilities
//! - When combined with [`JsonReporter`](crate::reporter::JsonReporter) for machine-readable output

use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;

use crate::{
    histogram::LatencyHistogram,
    report::{BenchReport, IterReport},
    runner::BenchOpts,
    stats::IterStats,
};

/// A silent report collector that aggregates results without terminal output.
///
/// This collector is useful in headless environments (CI/CD, scripts) where
/// the interactive TUI is not needed. It still collects all statistics and
/// produces the same [`BenchReport`](crate::BenchReport) as [`TuiCollector`](super::TuiCollector).
///
/// The collector responds to `Ctrl+C` for graceful cancellation.
pub struct SilentCollector {
    bench_opts: BenchOpts,
    res_rx: UnboundedReceiver<Result<IterReport>>,
    cancel: CancellationToken,
}

impl SilentCollector {
    /// Create a new silent report collector.
    pub(crate) fn new(
        bench_opts: BenchOpts,
        res_rx: UnboundedReceiver<Result<IterReport>>,
        cancel: CancellationToken,
    ) -> Self {
        Self { bench_opts, res_rx, cancel }
    }
}

#[async_trait]
impl super::ReportCollector for SilentCollector {
    async fn run(&mut self) -> anyhow::Result<BenchReport> {
        let mut hist = LatencyHistogram::new();
        let mut stats = IterStats::new();
        let mut status_dist = HashMap::default();
        let mut error_dist = HashMap::default();

        loop {
            tokio::select! {
                biased;
                _ = tokio::signal::ctrl_c() => self.cancel.cancel(),
                r = self.res_rx.recv() => match r {
                    Some(Ok(report)) => {
                        *status_dist.entry(report.status).or_default() += 1;
                        hist.record(report.duration)?;
                        stats += &report;
                    }
                    Some(Err(e)) => *error_dist.entry(e.to_string()).or_default() += 1,
                    None => break,
                },
            }
        }

        let elapsed = self.bench_opts.clock.elapsed();
        let concurrency = self.bench_opts.concurrency;
        Ok(BenchReport { concurrency, hist, stats, status_dist, error_dist, elapsed })
    }
}
