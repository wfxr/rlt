//! Baseline storage operations (load/save).

use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use chrono::Utc;

use super::{
    Baseline, BaselineMetadata, BaselineName, BaselineResult, BenchConfig, Latency, LatencyStats,
    RateSummary, SCHEMA_VERSION, SerializableReport, Summary,
};
use crate::cli::BenchCli;
use crate::error::BaselineError;
use crate::histogram::PERCENTAGES;
use crate::report::BenchReport;
use crate::util::rate;

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
        .or_else(|_| {
            std::env::var("CARGO_TARGET_DIR").map(|d| PathBuf::from(d).join("rlt/baselines"))
        })
        .unwrap_or_else(|_| PathBuf::from("target/rlt/baselines"))
}

/// Load a baseline by name from the baseline directory.
pub fn load(baseline_dir: &Path, name: &BaselineName) -> BaselineResult<Baseline> {
    let path = baseline_dir.join(format!("{}.json", name));
    load_file(&path)
}

/// Load a baseline from a file path.
pub fn load_file(path: &Path) -> BaselineResult<Baseline> {
    let file = File::open(path)
        .map_err(|e| BaselineError::Open { path: path.to_path_buf(), source: e })?;
    let reader = BufReader::new(file);

    let baseline: Baseline = serde_json::from_reader(reader)
        .map_err(|e| BaselineError::Parse { path: path.to_path_buf(), source: e })?;

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
pub fn save(
    baseline_dir: &Path,
    name: &BaselineName,
    report: &BenchReport,
    cli: &BenchCli,
) -> BaselineResult<()> {
    let name = name.as_str();

    // Ensure directory exists
    fs::create_dir_all(baseline_dir)
        .map_err(|e| BaselineError::CreateDir { dir: baseline_dir.to_path_buf(), source: e })?;

    let baseline = create_baseline(name, report, cli);

    let path = baseline_dir.join(format!("{}.json", name));
    let temp_path = baseline_dir.join(format!("{}.json.tmp", name));

    // Write to temporary file
    {
        macro_rules! map_err {
            ($expr:expr, $variant:ident) => {
                $expr.map_err(|e| BaselineError::$variant { path: temp_path.clone(), source: e })?
            };
        }

        let file = map_err!(File::create(&temp_path), CreateTemp);
        let mut writer = BufWriter::new(file);
        map_err!(serde_json::to_writer_pretty(&mut writer, &baseline), Serialize);
        map_err!(writer.flush(), Flush);
        map_err!(writer.get_ref().sync_all(), Sync);
    }

    // Atomic rename
    fs::rename(&temp_path, &path).map_err(|e| BaselineError::Rename {
        from: temp_path,
        to: path,
        source: e,
    })?;

    Ok(())
}

/// Create a Baseline from a BenchReport and CLI options.
fn create_baseline(name: &str, report: &BenchReport, cli: &BenchCli) -> Baseline {
    let elapsed = report.elapsed.as_secs_f64();
    let overall = &report.stats.overall;

    // Build summary
    let summary = Summary {
        success_ratio: report.success_ratio(),
        total_time: elapsed,
        concurrency: report.concurrency,
        iters: RateSummary { total: overall.iters, rate: rate(overall.iters, elapsed) },
        items: RateSummary { total: overall.items, rate: rate(overall.items, elapsed) },
        bytes: RateSummary { total: overall.bytes, rate: rate(overall.bytes, elapsed) },
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
    fn test_resolve_baseline_dir_cli_override() {
        let cli_dir = PathBuf::from("/custom/path");
        let dir = resolve_baseline_dir(Some(&cli_dir));
        assert_eq!(dir, cli_dir);
    }
}
