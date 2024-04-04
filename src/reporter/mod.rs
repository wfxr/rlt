//! This module defines a trait for printing benchmark reports.
mod json;
mod text;

pub use json::JsonReporter;
pub use text::TextReporter;

use crate::report::BenchReport;

/// A trait for reporting benchmark results.
pub trait BenchReporter {
    /// Print the report to the given writer.
    fn print(&self, w: &mut dyn std::io::Write, report: &BenchReport) -> anyhow::Result<()>;
}
