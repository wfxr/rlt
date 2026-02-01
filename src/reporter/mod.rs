//! Report formatting and output for benchmark results.
//!
//! This module provides reporters that format and output benchmark results
//! in various formats.
//!
//! # Available Reporters
//!
//! - [`TextReporter`] - Human-readable colored text output with tables and histograms.
//!   Ideal for terminal viewing.
//! - [`JsonReporter`] - Machine-readable JSON output with all statistics.
//!   Ideal for CI/CD integration, data analysis, or piping to other tools.
//!
//! # Baseline Comparison
//!
//! Both reporters support optional baseline comparison. When a [`Comparison`] is
//! provided, the report will include delta analysis showing performance changes
//! relative to the baseline.
//!
//! # Example
//!
//! ```ignore
//! use rlt::reporter::{BenchReporter, TextReporter};
//!
//! let reporter = TextReporter;
//! let mut output = Vec::new();
//! reporter.print(&mut output, &report, None)?;
//! ```

mod json;
mod text;

pub use json::JsonReporter;
pub use text::TextReporter;

use crate::baseline::Comparison;
use crate::error::ReporterError;
use crate::report::BenchReport;

/// A trait for formatting and outputting benchmark reports.
///
/// Implementors convert a [`BenchReport`] into a specific output format
/// and write it to the provided writer.
pub trait BenchReporter {
    /// Formats and writes the benchmark report.
    ///
    /// # Arguments
    ///
    /// * `w` - The writer to output the formatted report to.
    /// * `report` - The benchmark report to format.
    /// * `comparison` - Optional baseline comparison data to include in the output.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the writer fails or if formatting encounters issues.
    fn print(
        &self,
        w: &mut dyn std::io::Write,
        report: &BenchReport,
        comparison: Option<&Comparison>,
    ) -> std::result::Result<(), ReporterError>;
}
