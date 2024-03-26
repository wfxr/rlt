use crate::{histogram::PERCENTAGES, report::BenchReport};

use super::BenchReporter;

use serde::Serialize;
use std::{collections::BTreeMap, io::Write};

pub struct JsonReporter;

impl BenchReporter for JsonReporter {
    fn print(&self, w: &mut dyn Write, report: &BenchReport) -> anyhow::Result<()> {
        let elapsed = report.elapsed.as_secs_f64();
        let counter = &report.stats.counter;
        let summary = Summary {
            success_ratio: report.success_ratio(),
            total_time: elapsed,
            concurrency: report.concurrency,

            iters: ItersSummary {
                total: counter.iters,
                rate: counter.iters as f64 / elapsed,
                bytes_per_iter: counter.bytes.checked_div(counter.iters),
            },

            items: ItemsSummary {
                total: counter.items,
                rate: counter.items as f64 / elapsed,
                items_per_iter: counter.items as f64 / counter.iters as f64,
                bytes_per_item: counter.bytes.checked_div(counter.items),
            },

            bytes: BytesSummary { total: counter.bytes, rate: counter.bytes as f64 / elapsed },
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
    #[serde(skip_serializing_if = "not_normal_f64")]
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
}

fn not_normal_f64(v: &f64) -> bool {
    !v.is_normal()
}
