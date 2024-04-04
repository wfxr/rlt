//! This module defines a trait for collecting iteration results.
mod silent;
mod tui;

use async_trait::async_trait;

pub use silent::SilentCollector;
pub use tui::TuiCollector;

use crate::report::BenchReport;

/// A trait for collecting iteration results.
#[async_trait]
pub trait ReportCollector: Send + Sync {
    /// Run the collector and generate a benchmark report.
    async fn run(&mut self) -> anyhow::Result<BenchReport>;
}
