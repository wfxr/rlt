//! Baseline storage operations (load/save).

use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::Context;
use chrono::Utc;

use crate::{cli::BenchCli, histogram::PERCENTAGES, report::BenchReport};

use super::{
    Baseline, BaselineMetadata, BenchConfig, Latency, LatencyStats, RateSummary, SCHEMA_VERSION, SerializableReport,
    Summary,
};

/// Resolve the baseline directory using the priority order:
/// 1. CLI flag (if specified)
/// 2. RLT_BASELINE_DIR environment variable (if set)
/// 3. ${CARGO_TARGET_DIR}/rlt/baselines (if CARGO_TARGET_DIR is set)
/// 4. target/rlt/baselines (default fallback)
pub fn resolve_baseline_dir(cli_dir: Option<&Path>) -> PathBuf {
    if let Some(dir) = cli_dir {
        return dir.to_path_buf();
    }
    std::env::var("RLT_BASELINE_DIR")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("CARGO_TARGET_DIR").map(|d| PathBuf::from(d).join("rlt/baselines")))
        .unwrap_or_else(|_| PathBuf::from("target/rlt/baselines"))
}

/// Load a baseline by name from the baseline directory.
pub fn load(baseline_dir: &Path, name: impl AsRef<str>) -> anyhow::Result<Baseline> {
    let path = baseline_dir.join(format!("{}.json", name.as_ref()));
    load_file(&path)
}

/// Load a baseline from a file path.
pub fn load_file(path: &Path) -> anyhow::Result<Baseline> {
    let file = File::open(path).with_context(|| format!("Failed to open baseline file: {}", path.display()))?;
    let reader = BufReader::new(file);

    let baseline: Baseline = serde_json::from_reader(reader)
        .with_context(|| format!("Failed to parse baseline file: {}", path.display()))?;

    #[cfg(feature = "tracing")]
    if baseline.schema_version > SCHEMA_VERSION {
        log::warn!(
            "Baseline was created with a newer schema version ({}), attempting best-effort parsing",
            baseline.schema_version
        );
    }

    Ok(baseline)
}

/// Save a benchmark report as a baseline.
///
/// Uses atomic write (write-to-temp-then-rename) to prevent corruption.
pub fn save(baseline_dir: &Path, name: impl AsRef<str>, report: &BenchReport, cli: &BenchCli) -> anyhow::Result<()> {
    let name = name.as_ref();

    // Ensure directory exists
    fs::create_dir_all(baseline_dir)
        .with_context(|| format!("Failed to create baseline directory: {}", baseline_dir.display()))?;

    let baseline = create_baseline(name, report, cli);

    let path = baseline_dir.join(format!("{}.json", name));
    let temp_path = baseline_dir.join(format!("{}.json.tmp", name));

    // Write to temporary file
    {
        let file = File::create(&temp_path)
            .with_context(|| format!("Failed to create temporary file: {}", temp_path.display()))?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, &baseline)
            .with_context(|| format!("Failed to serialize baseline: {}", temp_path.display()))?;
        writer.flush()?;
        writer.get_ref().sync_all()?;
    }

    // Atomic rename
    fs::rename(&temp_path, &path)
        .with_context(|| format!("Failed to rename {} to {}", temp_path.display(), path.display()))?;

    Ok(())
}

/// Create a Baseline from a BenchReport and CLI options.
fn create_baseline(name: &str, report: &BenchReport, cli: &BenchCli) -> Baseline {
    let elapsed = report.elapsed.as_secs_f64();
    let counter = &report.stats.counter;

    // Build summary
    let summary = Summary {
        success_ratio: report.success_ratio(),
        total_time: elapsed,
        concurrency: report.concurrency,
        iters: RateSummary { total: counter.iters, rate: counter.iters as f64 / elapsed },
        items: RateSummary { total: counter.items, rate: counter.items as f64 / elapsed },
        bytes: RateSummary { total: counter.bytes, rate: counter.bytes as f64 / elapsed },
    };

    // Build latency (if available)
    let latency = if report.hist.is_empty() {
        None
    } else {
        Some(Latency {
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
        })
    };

    let serializable_report = SerializableReport {
        summary,
        latency,
        status: report.status_dist.iter().map(|(k, &v)| (k.to_string(), v)).collect(),
        errors: report.error_dist.iter().map(|(k, &v)| (k.clone(), v)).collect(),
    };

    // Build bench config
    let bench_config = BenchConfig {
        concurrency: cli.concurrency.get(),
        duration_secs: cli.duration.map(|d| d.as_secs_f64()),
        iterations: cli.iterations.map(|n| n.get()),
        warmup: cli.warmup,
        #[cfg(feature = "rate_limit")]
        rate_limit: cli.rate.map(|r| r.get()),
        #[cfg(not(feature = "rate_limit"))]
        rate_limit: None,
        actual_duration_secs: elapsed,
    };

    Baseline {
        schema_version: SCHEMA_VERSION,
        metadata: BaselineMetadata {
            name: name.to_string(),
            created_at: Utc::now(),
            rlt_version: env!("CARGO_PKG_VERSION").to_string(),
            bench_config,
        },
        report: serializable_report,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baseline::BaselineName;

    #[test]
    fn test_baseline_name_valid() {
        assert!("v1.0".parse::<BaselineName>().is_ok());
        assert!("main".parse::<BaselineName>().is_ok());
        assert!("feature-branch".parse::<BaselineName>().is_ok());
        assert!("release_2.0".parse::<BaselineName>().is_ok());
        assert!("test123".parse::<BaselineName>().is_ok());
        assert!("a".parse::<BaselineName>().is_ok());
    }

    #[test]
    fn test_baseline_name_invalid() {
        assert!("".parse::<BaselineName>().is_err());
        assert!("foo/bar".parse::<BaselineName>().is_err());
        assert!("../escape".parse::<BaselineName>().is_err());
        assert!("name with spaces".parse::<BaselineName>().is_err());
        assert!("special@char".parse::<BaselineName>().is_err());
    }

    #[test]
    fn test_resolve_baseline_dir_default() {
        // Save current env vars
        let orig_rlt = std::env::var("RLT_BASELINE_DIR").ok();
        let orig_cargo = std::env::var("CARGO_TARGET_DIR").ok();

        // SAFETY: This test modifies environment variables, which can cause data races
        // in multi-threaded programs. This is acceptable in test code as tests are
        // typically run in isolation.
        unsafe {
            // Clear env vars for test
            std::env::remove_var("RLT_BASELINE_DIR");
            std::env::remove_var("CARGO_TARGET_DIR");
        }

        let dir = resolve_baseline_dir(None);
        assert_eq!(dir, PathBuf::from("target/rlt/baselines"));

        // Restore env vars
        // SAFETY: Same as above - test code modification of env vars.
        unsafe {
            if let Some(v) = orig_rlt {
                std::env::set_var("RLT_BASELINE_DIR", v);
            }
            if let Some(v) = orig_cargo {
                std::env::set_var("CARGO_TARGET_DIR", v);
            }
        }
    }

    #[test]
    fn test_resolve_baseline_dir_cli_override() {
        let cli_dir = PathBuf::from("/custom/path");
        let dir = resolve_baseline_dir(Some(&cli_dir));
        assert_eq!(dir, cli_dir);
    }
}
