//! Baseline comparison logic.

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::{report::BenchReport, util::rate};

use super::Baseline;

/// Metrics that can be used for regression detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, clap::ValueEnum, strum::Display)]
#[strum(serialize_all = "kebab-case")]
#[serde(rename_all = "kebab-case")]
pub enum RegressionMetric {
    /// Iterations per second.
    ItersRate,
    /// Items per second.
    ItemsRate,
    /// Bytes per second.
    BytesRate,
    /// Mean latency.
    LatencyMean,
    /// Median latency (p50).
    LatencyMedian,
    /// 90th percentile latency.
    LatencyP90,
    /// 99th percentile latency.
    LatencyP99,
    /// Maximum latency.
    LatencyMax,
    /// Success ratio.
    SuccessRatio,
}

impl RegressionMetric {
    /// Returns the display name for this metric in comparison tables.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::ItersRate => "Iters/s",
            Self::ItemsRate => "Items/s",
            Self::BytesRate => "Bytes/s",
            Self::SuccessRatio => "Success",
            Self::LatencyMean => "Avg",
            Self::LatencyMedian => "Med",
            Self::LatencyP90 => "p90",
            Self::LatencyP99 => "p99",
            Self::LatencyMax => "Max",
        }
    }
}

/// Delta value representation - either percentage change or percentage points.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DeltaValue {
    /// Percentage change: (current - baseline) / baseline * 100.
    Percent(f64),
    /// Percentage points (absolute difference * 100, for ratios like success_ratio).
    Points(f64),
}

/// Status of a metric comparison.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeltaStatus {
    /// Performance improved.
    Improved,
    /// Performance regressed.
    Regressed,
    /// Within noise threshold (no significant change).
    Unchanged,
}

/// Comparison result for a single metric.
#[derive(Debug, Clone, Serialize)]
pub struct Delta {
    /// Current value.
    pub current: f64,
    /// Baseline value.
    pub baseline: f64,
    /// Ratio of current to baseline (current / baseline).
    /// None if baseline is 0 and current is non-zero (infinite ratio).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ratio: Option<f64>,
    /// The delta value (either percent or points).
    /// None if baseline is 0 (cannot compute percentage).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<DeltaValue>,
    /// Comparison status.
    pub status: DeltaStatus,
}

/// Latency comparison results.
#[derive(Debug, Clone, Serialize)]
pub struct LatencyDeltas {
    /// Mean latency comparison.
    pub mean: Delta,
    /// Median latency comparison.
    pub median: Delta,
    /// 90th percentile latency comparison (if available in baseline).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p90: Option<Delta>,
    /// 99th percentile latency comparison (if available in baseline).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p99: Option<Delta>,
    /// Maximum latency comparison.
    pub max: Delta,
}

/// Throughput comparison results.
#[derive(Debug, Clone, Serialize)]
pub struct ThroughputDeltas {
    /// Iterations per second comparison.
    pub iters_rate: Delta,
    /// Items per second comparison (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items_rate: Option<Delta>,
    /// Bytes per second comparison (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_rate: Option<Delta>,
}

/// Overall verdict of the comparison.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// All regression metrics improved or unchanged, at least one improved.
    Improved,
    /// All regression metrics regressed or unchanged, at least one regressed.
    Regressed,
    /// All regression metrics unchanged.
    Unchanged,
    /// Some metrics improved, some regressed (treated as regression for --fail-on-regression).
    Mixed,
}

/// Overall comparison result.
#[derive(Debug, Clone, Serialize)]
pub struct Comparison {
    /// Name of the baseline.
    pub baseline_name: String,
    /// When the baseline was created.
    pub baseline_created_at: DateTime<Utc>,
    /// Schema version of the baseline.
    pub schema_version: u32,
    /// Noise threshold used for comparison.
    pub noise_threshold_percent: f64,
    /// Metrics considered for verdict calculation.
    pub regression_metrics: Vec<RegressionMetric>,
    /// Metrics that were skipped (unavailable for comparison).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skipped_metrics: Vec<RegressionMetric>,
    /// Overall verdict.
    pub verdict: Verdict,
    /// Throughput comparisons.
    pub throughput: ThroughputDeltas,
    /// Latency comparisons (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency: Option<LatencyDeltas>,
    /// Success ratio comparison.
    pub success_ratio: Delta,
}

/// Compare a benchmark report against a baseline.
pub fn compare(
    report: &BenchReport,
    baseline: &Baseline,
    noise_threshold: f64,
    regression_metrics: &[RegressionMetric],
) -> Comparison {
    let elapsed = report.elapsed.as_secs_f64();
    let counter = &report.stats.counter;
    let baseline_summary = &baseline.report.summary;

    // Calculate throughput deltas
    let throughput = ThroughputDeltas {
        iters_rate: calculate_throughput_delta(
            rate(counter.iters, elapsed),
            baseline_summary.iters.rate,
            noise_threshold,
        ),
        items_rate: if counter.items > 0 || baseline_summary.items.total > 0 {
            Some(calculate_throughput_delta(
                rate(counter.items, elapsed),
                baseline_summary.items.rate,
                noise_threshold,
            ))
        } else {
            None
        },
        bytes_rate: if counter.bytes > 0 || baseline_summary.bytes.total > 0 {
            Some(calculate_throughput_delta(
                rate(counter.bytes, elapsed),
                baseline_summary.bytes.rate,
                noise_threshold,
            ))
        } else {
            None
        },
    };

    // Calculate latency deltas (if available)
    let latency = if report.hist.is_empty() {
        None
    } else {
        baseline.report.latency.as_ref().map(|baseline_latency| LatencyDeltas {
            mean: calculate_latency_delta(
                report.hist.mean().as_secs_f64(),
                baseline_latency.stats.mean,
                noise_threshold,
            ),
            median: calculate_latency_delta(
                report.hist.median().as_secs_f64(),
                baseline_latency.stats.median,
                noise_threshold,
            ),
            p90: baseline_latency.percentiles.get("p90").map(|&v| {
                calculate_latency_delta(report.hist.value_at_quantile(0.90).as_secs_f64(), v, noise_threshold)
            }),
            p99: baseline_latency.percentiles.get("p99").map(|&v| {
                calculate_latency_delta(report.hist.value_at_quantile(0.99).as_secs_f64(), v, noise_threshold)
            }),
            max: calculate_latency_delta(
                report.hist.max().as_secs_f64(),
                baseline_latency.stats.max,
                noise_threshold,
            ),
        })
    };

    // Calculate success ratio delta
    let success_ratio =
        calculate_success_ratio_delta(report.success_ratio(), baseline_summary.success_ratio, noise_threshold);

    // Calculate verdict based on regression metrics
    let (verdict, skipped) = calculate_verdict(&throughput, latency.as_ref(), &success_ratio, regression_metrics);

    Comparison {
        baseline_name: baseline.metadata.name.clone(),
        baseline_created_at: baseline.metadata.created_at,
        schema_version: baseline.schema_version,
        noise_threshold_percent: noise_threshold,
        regression_metrics: regression_metrics.to_vec(),
        skipped_metrics: skipped,
        verdict,
        throughput,
        latency,
        success_ratio,
    }
}

/// Calculate delta for a throughput metric (higher is better).
fn calculate_throughput_delta(current: f64, baseline: f64, noise_threshold: f64) -> Delta {
    calculate_delta(current, baseline, noise_threshold, true)
}

/// Calculate delta for a latency metric (lower is better).
fn calculate_latency_delta(current: f64, baseline: f64, noise_threshold: f64) -> Delta {
    calculate_delta(current, baseline, noise_threshold, false)
}

/// Calculate delta for success ratio (higher is better, uses percentage points).
fn calculate_success_ratio_delta(current: f64, baseline: f64, noise_threshold: f64) -> Delta {
    let ratio = match (baseline == 0.0, current == 0.0) {
        (true, true) => Some(1.0),
        (true, false) => None, // Infinite ratio
        _ => Some(current / baseline),
    };

    // Success ratio uses percentage points (absolute difference * 100)
    let delta_points = (current - baseline) * 100.0;
    let delta = Some(DeltaValue::Points(delta_points));

    // For success ratio, changes within noise_threshold percentage points are unchanged
    let status = match delta_points {
        d if d.abs() <= noise_threshold => DeltaStatus::Unchanged,
        d if d > 0.0 => DeltaStatus::Improved, // Higher success ratio is better
        _ => DeltaStatus::Regressed,
    };

    Delta { current, baseline, ratio, delta, status }
}

/// Generic delta calculation.
///
/// - `higher_is_better`: true for throughput metrics, false for latency metrics.
fn calculate_delta(current: f64, baseline: f64, noise_threshold: f64, higher_is_better: bool) -> Delta {
    // Handle edge case: both zero
    if baseline == 0.0 && current == 0.0 {
        return Delta {
            current,
            baseline,
            ratio: Some(1.0),
            delta: Some(DeltaValue::Percent(0.0)),
            status: DeltaStatus::Unchanged,
        };
    }

    // Calculate ratio and delta (None if baseline is 0)
    let (ratio, delta) = if baseline == 0.0 {
        (None, None)
    } else {
        let r = current / baseline;
        let d = (current - baseline) / baseline * 100.0;
        (Some(r), Some(DeltaValue::Percent(d)))
    };

    // Determine status based on ratio and noise threshold
    let status = match ratio {
        Some(r) => {
            let percent_change = (r - 1.0).abs() * 100.0;
            if percent_change <= noise_threshold {
                DeltaStatus::Unchanged
            } else {
                // For throughput (higher_is_better=true): ratio > 1 means improved
                // For latency (higher_is_better=false): ratio < 1 means improved
                let is_improved = if higher_is_better { r > 1.0 } else { r < 1.0 };
                if is_improved {
                    DeltaStatus::Improved
                } else {
                    DeltaStatus::Regressed
                }
            }
        }
        // baseline == 0, current != 0: throughput improved, latency regressed
        None => {
            if higher_is_better {
                DeltaStatus::Improved
            } else {
                DeltaStatus::Regressed
            }
        }
    };

    Delta { current, baseline, ratio, delta, status }
}

/// Calculate the overall verdict based on regression metrics.
///
/// Returns the verdict and a list of metrics that were skipped (unavailable).
fn calculate_verdict(
    throughput: &ThroughputDeltas,
    latency: Option<&LatencyDeltas>,
    success_ratio: &Delta,
    metrics: &[RegressionMetric],
) -> (Verdict, Vec<RegressionMetric>) {
    let mut statuses: Vec<DeltaStatus> = Vec::new();
    let mut skipped: Vec<RegressionMetric> = Vec::new();

    for metric in metrics {
        let status = match metric {
            RegressionMetric::ItersRate => Some(throughput.iters_rate.status),
            RegressionMetric::ItemsRate => throughput.items_rate.as_ref().map(|d| d.status),
            RegressionMetric::BytesRate => throughput.bytes_rate.as_ref().map(|d| d.status),
            RegressionMetric::LatencyMean => latency.map(|l| l.mean.status),
            RegressionMetric::LatencyMedian => latency.map(|l| l.median.status),
            RegressionMetric::LatencyP90 => latency.and_then(|l| l.p90.as_ref().map(|d| d.status)),
            RegressionMetric::LatencyP99 => latency.and_then(|l| l.p99.as_ref().map(|d| d.status)),
            RegressionMetric::LatencyMax => latency.map(|l| l.max.status),
            RegressionMetric::SuccessRatio => Some(success_ratio.status),
        };

        match status {
            Some(s) => statuses.push(s),
            None => skipped.push(*metric),
        }
    }

    let has_improved = statuses.contains(&DeltaStatus::Improved);
    let has_regressed = statuses.contains(&DeltaStatus::Regressed);

    let verdict = match (has_improved, has_regressed) {
        (true, true) => Verdict::Mixed,
        (true, false) => Verdict::Improved,
        (false, true) => Verdict::Regressed,
        (false, false) => Verdict::Unchanged,
    };

    (verdict, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_throughput_delta_improved() {
        let delta = calculate_throughput_delta(1100.0, 1000.0, 1.0);
        assert_eq!(delta.status, DeltaStatus::Improved);
        assert!((delta.ratio.unwrap() - 1.1).abs() < 0.001);
    }

    #[test]
    fn test_calculate_throughput_delta_regressed() {
        let delta = calculate_throughput_delta(900.0, 1000.0, 1.0);
        assert_eq!(delta.status, DeltaStatus::Regressed);
        assert!((delta.ratio.unwrap() - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_calculate_throughput_delta_unchanged() {
        let delta = calculate_throughput_delta(1005.0, 1000.0, 1.0);
        assert_eq!(delta.status, DeltaStatus::Unchanged);
    }

    #[test]
    fn test_calculate_latency_delta_improved() {
        // Lower latency is better
        let delta = calculate_latency_delta(0.9, 1.0, 1.0);
        assert_eq!(delta.status, DeltaStatus::Improved);
    }

    #[test]
    fn test_calculate_latency_delta_regressed() {
        // Higher latency is worse
        let delta = calculate_latency_delta(1.1, 1.0, 1.0);
        assert_eq!(delta.status, DeltaStatus::Regressed);
    }

    #[test]
    fn test_calculate_verdict_improved() {
        let throughput = ThroughputDeltas {
            iters_rate: Delta {
                current: 1100.0,
                baseline: 1000.0,
                ratio: Some(1.1),
                delta: Some(DeltaValue::Percent(10.0)),
                status: DeltaStatus::Improved,
            },
            items_rate: None,
            bytes_rate: None,
        };
        let success_ratio = Delta {
            current: 1.0,
            baseline: 1.0,
            ratio: Some(1.0),
            delta: Some(DeltaValue::Points(0.0)),
            status: DeltaStatus::Unchanged,
        };

        let (verdict, skipped) = calculate_verdict(&throughput, None, &success_ratio, &[RegressionMetric::ItersRate]);
        assert_eq!(verdict, Verdict::Improved);
        assert!(skipped.is_empty());
    }

    #[test]
    fn test_calculate_verdict_mixed() {
        let throughput = ThroughputDeltas {
            iters_rate: Delta {
                current: 1100.0,
                baseline: 1000.0,
                ratio: Some(1.1),
                delta: Some(DeltaValue::Percent(10.0)),
                status: DeltaStatus::Improved,
            },
            items_rate: None,
            bytes_rate: None,
        };
        let success_ratio = Delta {
            current: 0.95,
            baseline: 0.99,
            ratio: Some(0.9596),
            delta: Some(DeltaValue::Points(-4.0)),
            status: DeltaStatus::Regressed,
        };

        let (verdict, skipped) = calculate_verdict(
            &throughput,
            None,
            &success_ratio,
            &[RegressionMetric::ItersRate, RegressionMetric::SuccessRatio],
        );
        assert_eq!(verdict, Verdict::Mixed);
        assert!(skipped.is_empty());
    }
}
