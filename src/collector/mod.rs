//! Report collection for benchmark results.
//!
//! This module provides the infrastructure for collecting iteration results
//! from benchmark workers and aggregating them into a final report.
//!
//! # Overview
//!
//! Collectors receive [`IterReport`](crate::IterReport) results from workers via a channel,
//! aggregate statistics, and produce a final [`BenchReport`] when the benchmark completes.
//!
//! # Available Collectors
//!
//! - [`TuiCollector`] - Interactive terminal UI with real-time statistics, histograms,
//!   and progress visualization. Supports pausing and keyboard controls.
//! - [`SilentCollector`] - Headless collector for CI/CD environments or scripted use.
//!   Collects results without any terminal output.
//!
//! # Example
//!
//! Collectors are typically created and managed by the benchmark runner, but can
//! be used directly:
//!
//! ```ignore
//! let mut collector = SilentCollector::new(bench_opts, result_rx, cancel_token);
//! let report = collector.run().await?;
//! ```

mod silent;
mod tui;

use async_trait::async_trait;
pub use silent::SilentCollector;
pub use tui::TuiCollector;

use crate::Result;
use crate::report::BenchReport;

/// A trait for collecting iteration results and generating benchmark reports.
///
/// Implementors receive iteration results from workers, track statistics,
/// and produce a final aggregated report when the benchmark completes.
#[async_trait]
pub trait ReportCollector: Send + Sync {
    /// Runs the collector and generates a benchmark report.
    ///
    /// This method blocks until all workers have completed and the result
    /// channel is closed, then returns the aggregated benchmark report.
    ///
    /// # Errors
    ///
    /// Returns an error if the collector encounters issues during collection
    /// (e.g., histogram overflow, I/O errors for TUI).
    async fn run(&mut self) -> Result<BenchReport>;
}
