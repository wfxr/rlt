//! Baseline support for benchmark reports.
//!
//! This module provides functionality for saving benchmark results as baselines
//! and comparing future runs against them.

mod compare;
mod storage;

pub use compare::{
    Comparison, Delta, DeltaStatus, LatencyDeltas, RegressionMetric, ThroughputDeltas, Verdict, compare,
};
pub use storage::{load, load_file, resolve_baseline_dir, save};

use std::{collections::BTreeMap, fmt, str::FromStr};

/// A validated baseline name.
///
/// Baseline names must match the pattern `[a-zA-Z0-9_.-]+`:
/// - Allowed: `v1.0`, `main`, `feature-branch`, `release_2.0`
/// - Rejected: `foo/bar`, `../escape`, `name with spaces`, empty string
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineName(String);

impl BaselineName {
    /// Returns the baseline name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for BaselineName {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err("baseline name cannot be empty".to_string());
        }
        if !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return Err(format!(
                "invalid baseline name '{}': must contain only [a-zA-Z0-9_.-]",
                s
            ));
        }
        Ok(BaselineName(s.to_string()))
    }
}

impl fmt::Display for BaselineName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for BaselineName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Current schema version for baseline files.
pub(crate) const SCHEMA_VERSION: u32 = 1;

/// Benchmark configuration for comparability checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BenchConfig {
    /// Number of concurrent workers.
    pub concurrency: u32,
    /// Duration in seconds (if set).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<f64>,
    /// Number of iterations (if set).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iterations: Option<u64>,
    /// Number of warmup iterations.
    pub warmup: u64,
    /// Rate limit in iterations per second (if set).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<u32>,
    /// Actual elapsed time of the benchmark.
    pub actual_duration_secs: f64,
}

/// Baseline metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BaselineMetadata {
    /// Name of the baseline.
    pub name: String,
    /// When the baseline was created.
    pub created_at: DateTime<Utc>,
    /// Version of rlt that created this baseline.
    pub rlt_version: String,
    /// Benchmark configuration.
    pub bench_config: BenchConfig,
}

/// Summary statistics for serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Summary {
    /// Success ratio (0.0 to 1.0).
    pub success_ratio: f64,
    /// Total elapsed time in seconds.
    pub total_time: f64,
    /// Number of concurrent workers.
    pub concurrency: u32,
    /// Iteration statistics.
    pub iters: RateSummary,
    /// Item statistics.
    pub items: RateSummary,
    /// Byte statistics.
    pub bytes: RateSummary,
}

/// Rate-based summary (total and rate per second).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RateSummary {
    /// Total count.
    pub total: u64,
    /// Rate per second.
    pub rate: f64,
}

/// Latency statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LatencyStats {
    /// Minimum latency in seconds.
    pub min: f64,
    /// Maximum latency in seconds.
    pub max: f64,
    /// Mean latency in seconds.
    pub mean: f64,
    /// Median latency in seconds.
    pub median: f64,
    /// Standard deviation in seconds.
    pub stdev: f64,
}

/// Latency data including stats, percentiles, and histogram.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Latency {
    /// Basic statistics.
    pub stats: LatencyStats,
    /// Percentile values (e.g., "p90" -> 0.003123).
    pub percentiles: BTreeMap<String, f64>,
    /// Histogram buckets.
    pub histogram: BTreeMap<String, u64>,
}

/// Serializable form of BenchReport for JSON storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SerializableReport {
    /// Summary statistics.
    pub summary: Summary,
    /// Latency data (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency: Option<Latency>,
    /// Status distribution.
    pub status: BTreeMap<String, u64>,
    /// Error distribution.
    pub errors: BTreeMap<String, u64>,
}

/// Complete baseline data (metadata + report).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    /// Schema version for forward compatibility.
    pub(crate) schema_version: u32,
    /// Baseline metadata.
    pub(crate) metadata: BaselineMetadata,
    /// Report data (flattened into the baseline).
    #[serde(flatten)]
    pub(crate) report: SerializableReport,
}

impl Baseline {
    /// Validate that this baseline is comparable with the given CLI settings.
    ///
    /// This should be called **before** running the benchmark to fail fast on incompatible baselines.
    pub fn validate(&self, cli: &crate::cli::BenchCli) -> anyhow::Result<()> {
        let config = &self.metadata.bench_config;

        // Concurrency mismatch is an error - results would not be comparable
        if cli.concurrency.get() != config.concurrency {
            anyhow::bail!(
                "Concurrency mismatch: current={}, baseline={}. Results are not comparable.",
                cli.concurrency.get(),
                config.concurrency
            );
        }

        // Rate limit mismatch is an error - directly affects throughput
        #[cfg(feature = "rate_limit")]
        {
            let current_rate = cli.rate.map(|r| r.get());
            if current_rate != config.rate_limit {
                anyhow::bail!(
                    "Rate limit mismatch: current={:?}, baseline={:?}. Results are not comparable.",
                    current_rate,
                    config.rate_limit
                );
            }
        }

        Ok(())
    }
}
