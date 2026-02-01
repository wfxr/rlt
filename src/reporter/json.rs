//! JSON reporter for machine-readable benchmark output.
//!
//! This module provides [`JsonReporter`], which formats benchmark results
//! as pretty-printed JSON suitable for parsing by other tools.
//!
//! # Output Structure
//!
//! The JSON output includes:
//! - `summary`: Success ratio, total time, concurrency, and throughput metrics
//! - `latency`: Statistics, percentiles, and histogram (omitted if no iterations)
//! - `status`: Status code distribution
//! - `errors`: Error message distribution (may be empty)
//! - `comparison`: Baseline comparison data (if provided)
//!
//! # Use Cases
//!
//! - CI/CD pipelines for automated performance regression detection
//! - Data analysis and visualization tools
//! - Integration with monitoring systems
//! - Historical performance tracking

use crate::{baseline::Comparison, histogram::PERCENTAGES, report::BenchReport, util::rate};

use super::BenchReporter;

use serde::Serialize;

use super::ReporterResult;
use std::{collections::BTreeMap, io::Write};

/// A JSON reporter that outputs machine-readable benchmark results.
///
/// This reporter formats results as pretty-printed JSON, making it ideal
/// for automated processing, data analysis, or integration with other tools.
///
/// # Example
///
/// ```ignore
/// use rlt::reporter::{BenchReporter, JsonReporter};
///
/// let reporter = JsonReporter;
/// let mut output = Vec::new();
/// reporter.print(&mut output, &report, None)?;
/// let json_string = String::from_utf8(output)?;
/// ```
pub struct JsonReporter;

impl BenchReporter for JsonReporter {
    fn print(&self, w: &mut dyn Write, report: &BenchReport, comparison: Option<&Comparison>) -> ReporterResult<()> {
        let elapsed = report.elapsed.as_secs_f64();
        let overall = &report.stats.overall;
        let summary = Summary {
            success_ratio: report.success_ratio(),
            total_time: elapsed,
            concurrency: report.concurrency,

            iters: ItersSummary {
                total: overall.iters,
                rate: rate(overall.iters, elapsed),
                bytes_per_iter: overall.bytes.checked_div(overall.iters),
            },

            items: ItemsSummary {
                total: overall.items,
                rate: rate(overall.items, elapsed),
                items_per_iter: if overall.iters > 0 {
                    overall.items as f64 / overall.iters as f64
                } else {
                    0.0
                },
                bytes_per_item: overall.bytes.checked_div(overall.items),
            },

            bytes: BytesSummary { total: overall.bytes, rate: rate(overall.bytes, elapsed) },
        };

        let latency = if report.hist.is_empty() {
            None
        } else {
            Latency {
                stats: LatencyStats {
                    min: report.hist.min().as_secs_f64(),
                    max: report.hist.max().as_secs_f64(),
                    mean: report.hist.mean().as_secs_f64(),
                    median: report.hist.median().as_secs_f64(),
                    stdev: report.hist.stdev().as_secs_f64(),
                },
                percentiles: report
                    .hist
                    .percentiles(PERCENTAGES)
                    .map(|(p, v)| (format!("p{p}"), v.as_secs_f64()))
                    .collect(),
                histogram: report
                    .hist
                    .quantiles()
                    .map(|(k, v)| (k.as_secs_f64().to_string(), v))
                    .collect(),
            }
            .into()
        };

        serde_json::to_writer_pretty(
            &mut *w,
            &Report {
                summary,
                latency,
                status: report.status_dist.iter().map(|(k, &v)| (k.to_string(), v)).collect(),
                errors: report.error_dist.iter().map(|(k, &v)| (k.clone(), v)).collect(),
                comparison: comparison.cloned(),
            },
        )?;

        writeln!(w)?;
        Ok(())
    }
}

#[derive(Serialize)]
struct Summary {
    success_ratio: f64,
    total_time: f64,
    concurrency: u32,

    iters: ItersSummary,
    items: ItemsSummary,
    bytes: BytesSummary,
}

#[derive(Serialize)]
struct ItersSummary {
    total: u64,
    rate: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    bytes_per_iter: Option<u64>,
}

#[derive(Serialize)]
struct ItemsSummary {
    total: u64,
    rate: f64,
    #[serde(skip_serializing_if = "is_not_finite_f64")]
    items_per_iter: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    bytes_per_item: Option<u64>,
}

#[derive(Serialize)]
struct BytesSummary {
    total: u64,
    rate: f64,
}

#[derive(Serialize)]
pub struct LatencyStats {
    min: f64,
    max: f64,
    mean: f64,
    median: f64,
    stdev: f64,
}

#[derive(Serialize)]
pub struct Latency {
    stats: LatencyStats,
    percentiles: BTreeMap<String, f64>,
    histogram: BTreeMap<String, u64>,
}

#[derive(Serialize)]
struct Report {
    summary: Summary,
    #[serde(skip_serializing_if = "Option::is_none")]
    latency: Option<Latency>,
    status: BTreeMap<String, u64>,
    errors: BTreeMap<String, u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    comparison: Option<Comparison>,
}

fn is_not_finite_f64(v: &f64) -> bool {
    !v.is_finite()
}
