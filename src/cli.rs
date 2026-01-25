//! This module provides a CLI interface for the benchmark tool.
//!
//! Usually you can embed [`BenchCli`] into your own CLI struct.
//!
//! # Examples
//!
//! ```no_run
//! use clap::Parser;
//! use rlt::cli::BenchCli;
//!
//! #[derive(Parser, Clone)]
//! pub struct Opts {
//!     /// Target URL.
//!     pub url: String,
//!
//!     /// Embed BenchOpts into this Opts.
//!     #[command(flatten)]
//!     pub bench_opts: BenchCli,
//! }
//! ```
//!
//! You can also use the provided macros `bench_cli!`:
//! ```no_run
//! rlt::bench_cli!(Opts, {
//!    /// Target URL.
//!     pub url: String,
//! });
//! ```
//!
//! The above example will generate a CLI struct with `url` and all the options
//! from `BenchCli`:
//!
//! ```shell
//! $ mybench --help
//! Usage: mybench [OPTIONS] <URL>
//!
//! Arguments:
//!   <URL>
//!           Target URL
//!
//! Options:
//!   -c, --concurrency <CONCURRENCY>
//!           Number of workers to run concurrently
//!
//!           [default: 1]
//!
//!   -n, --iterations <ITERATIONS>
//!           Number of iterations
//!
//!           When set, benchmark stops after reaching the number of iterations.
//!
//!   -d, --duration <DURATION>
//!           Duration to run the benchmark
//!
//!           When set, benchmark stops after reaching the duration.
//!
//!           Examples: -d 10s, -d 5m, -d 1h
//!
//!   -w, --warmup <WARMUP>
//!           Number of warm-up iterations to run before the main benchmark
//!
//!           Warm-up iterations are not included in the final benchmark results.
//!
//!   -r, --rate <RATE>
//!           Rate limit for benchmarking, in iterations per second (ips)
//!
//!           When set, benchmark will try to run at the specified rate.
//!
//!   -q, --quiet
//!           Run benchmark in quiet mode
//!
//!           Implies --collector silent.
//!
//!       --collector <COLLECTOR>
//!           Collector for the benchmark
//!
//!           Possible values:
//!           - tui:    TUI based collector
//!           - silent: Collector that does not print anything
//!
//!       --fps <FPS>
//!           Refresh rate for the tui collector, in frames per second (fps)
//!
//!           [default: 32]
//!
//!   -o, --output <OUTPUT>
//!           Output format for the report
//!
//!           [default: text]
//!
//!           Possible values:
//!           - text: Report in plain text format
//!           - json: Report in JSON format
//!
//!   -h, --help
//!           Print help (see a summary with '-h')
use std::{
    fs::File,
    io::stdout,
    num::{NonZeroU8, NonZeroU32, NonZeroU64},
    path::PathBuf,
};

use clap::{
    ArgGroup, Parser, ValueEnum,
    builder::{
        Styles,
        styling::{AnsiColor, Effects},
    },
};
use crossterm::tty::IsTty;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

use crate::{
    baseline::{self, BaselineName, RegressionMetric, Verdict},
    clock::Clock,
    collector::{ReportCollector, SilentCollector, TuiCollector},
    reporter::{BenchReporter, JsonReporter, TextReporter},
    runner::{BenchOpts, BenchSuite, Runner},
};

/// Error indicating a performance regression was detected.
///
/// This error is returned by [`run`] when `--fail-on-regression` is set
/// and the comparison verdict is `Regressed` or `Mixed`.
#[derive(Debug, Clone)]
pub struct RegressionError {
    /// The comparison verdict that triggered this error.
    pub verdict: Verdict,
    /// The name of the baseline used for comparison.
    pub baseline: String,
}

impl std::fmt::Display for RegressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Performance regression detected: {} (baseline: {})",
            self.verdict, self.baseline
        )
    }
}

impl std::error::Error for RegressionError {}

/// Default regression metrics for baseline comparison.
const DEFAULT_REGRESSION_METRICS: &[RegressionMetric] = &[
    RegressionMetric::ItersRate,
    RegressionMetric::LatencyMean,
    RegressionMetric::LatencyP90,
    RegressionMetric::LatencyP99,
    RegressionMetric::SuccessRatio,
];

#[derive(Parser, Clone, Debug)]
#[clap(
    styles(Styles::styled()
        .header(AnsiColor::Yellow.on_default() | Effects::BOLD)
        .usage(AnsiColor::Yellow.on_default() | Effects::BOLD)
        .literal(AnsiColor::Green.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Cyan.on_default())
    ),
    group = ArgGroup::new("baseline_source").args(["baseline", "baseline_file"]),
)]
#[allow(missing_docs)]
pub struct BenchCli {
    /// Number of workers to run concurrently
    #[clap(long, short = 'c', default_value = "1")]
    pub concurrency: NonZeroU32,

    /// Number of iterations
    ///
    /// When set, benchmark stops after reaching the number of iterations.
    #[clap(long, short = 'n')]
    pub iterations: Option<NonZeroU64>,

    /// Duration to run the benchmark
    ///
    /// When set, benchmark stops after reaching the duration.
    ///
    /// Examples: -d 10s, -d 5m, -d 1h
    #[clap(long, short = 'd')]
    pub duration: Option<humantime::Duration>,

    /// Number of warm-up iterations to run before the main benchmark
    ///
    /// Warm-up iterations are not included in the final benchmark results.
    #[clap(long, short = 'w', default_value_t = 0)]
    pub warmup: u64,

    #[cfg(feature = "rate_limit")]
    /// Rate limit for benchmarking, in iterations per second (ips)
    ///
    /// When set, benchmark will try to run at the specified rate.
    #[clap(long, short = 'r')]
    pub rate: Option<NonZeroU32>,

    /// Run benchmark in quiet mode
    ///
    /// Implies --collector silent.
    #[clap(long, short = 'q')]
    pub quiet: bool,

    /// Collector for the benchmark
    #[clap(long, value_enum, ignore_case = true)]
    pub collector: Option<Collector>,

    /// Refresh rate for the tui collector, in frames per second (fps)
    #[clap(long, default_value = "32")]
    pub fps: NonZeroU8,

    /// Quit the benchmark manually
    ///
    /// Only works with the TUI collector.
    #[clap(long)]
    pub quit_manually: bool,

    /// Output format for the report
    #[clap(short, long, value_enum, default_value_t = ReportFormat::Text, ignore_case = true)]
    pub output: ReportFormat,

    /// Output file path for the report
    ///
    /// When set, the report will be written to the specified file instead of stdout.
    #[clap(long, short = 'O')]
    pub output_file: Option<PathBuf>,

    /// Save benchmark results as a named baseline
    ///
    /// Can be combined with --baseline to compare and then save.
    /// Name must match pattern: [a-zA-Z0-9_.-]+ (no path separators or special chars)
    #[clap(long)]
    pub save_baseline: Option<BaselineName>,

    /// Compare against a named baseline from baseline directory
    ///
    /// Can be combined with --save-baseline (compare first, then save)
    #[clap(long, conflicts_with = "baseline_file")]
    pub baseline: Option<BaselineName>,

    /// Load baseline from a JSON file for comparison
    ///
    /// The file must be a baseline JSON generated by --save-baseline.
    #[clap(long, conflicts_with = "baseline")]
    pub baseline_file: Option<PathBuf>,

    /// Directory for storing baselines
    ///
    /// Priority: CLI flag > RLT_BASELINE_DIR > ${CARGO_TARGET_DIR}/rlt/baselines > target/rlt/baselines
    #[clap(long)]
    pub baseline_dir: Option<PathBuf>,

    /// Noise threshold for comparison (percentage, e.g., 1.0 means 1%)
    ///
    /// Changes within this threshold are considered noise and reported as "unchanged".
    #[clap(long, default_value = "1.0", value_parser = parse_noise_threshold)]
    pub noise_threshold: f64,

    /// Fail the benchmark if regression is detected (for CI/CD integration)
    ///
    /// Returns an error if verdict is 'regressed' or 'mixed'.
    #[clap(long, requires = "baseline_source")]
    pub fail_on_regression: bool,

    /// Metrics to consider for verdict calculation and regression detection
    #[clap(long, value_delimiter = ',', default_values_t = DEFAULT_REGRESSION_METRICS)]
    pub regression_metrics: Vec<RegressionMetric>,
}

impl BenchCli {
    pub(crate) fn bench_opts(&self, clock: Clock) -> BenchOpts {
        BenchOpts {
            clock,
            concurrency: self.concurrency.get(),
            iterations: self.iterations.map(|n| n.get()),
            duration: self.duration.map(|d| d.into()),
            warmups: self.warmup,
            #[cfg(feature = "rate_limit")]
            rate: self.rate,
        }
    }

    /// Get the actual collector type.
    pub fn collector(&self) -> Collector {
        match self.collector {
            Some(collector) => collector,
            None if self.quiet || !stdout().is_tty() => Collector::Silent,
            _ => Collector::Tui,
        }
    }
}

fn parse_noise_threshold(s: &str) -> Result<f64, String> {
    let v: f64 = s.parse().map_err(|e| format!("{e}"))?;
    if !v.is_finite() {
        return Err("noise threshold must be a finite number".to_string());
    }
    if v < 0.0 {
        return Err("noise threshold must be non-negative".to_string());
    }
    Ok(v)
}

/// The type of iteration report collector.
#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum Collector {
    /// TUI based collector. See [`TuiCollector`].
    Tui,

    /// Collector that does not print anything. See [`SilentCollector`].
    Silent,
}

/// Benchmark report format.
#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum ReportFormat {
    /// Report in plain text format. See [`TextReporter`].
    Text,

    /// Report in JSON format. See [`JsonReporter`].
    Json,
}

/// Run the benchmark with the given CLI options and benchmark suite.
pub async fn run<BS>(cli: BenchCli, bench_suite: BS) -> anyhow::Result<()>
where
    BS: BenchSuite + Send + Sync + 'static,
    BS::WorkerState: Send + Sync + 'static,
{
    // Resolve baseline directory
    let baseline_dir = baseline::resolve_baseline_dir(cli.baseline_dir.as_deref());

    // Load and validate baseline BEFORE running the benchmark (fail fast)
    let baseline = match (&cli.baseline, &cli.baseline_file) {
        (Some(name), _) => Some(baseline::load(&baseline_dir, name)?),
        (None, Some(path)) => Some(baseline::load_file(path)?),
        (None, None) => None,
    };
    baseline.as_ref().map(|b| b.validate(&cli)).transpose()?;

    // Now run the benchmark
    let (res_tx, res_rx) = mpsc::unbounded_channel();
    let (pause_tx, pause_rx) = watch::channel(false);
    let cancel = CancellationToken::new();

    // Create the clock in paused state - it will be resumed after all workers
    // complete setup and warmup, ensuring accurate timing for the main benchmark.
    let opts = cli.bench_opts(Clock::new_paused());
    let runner = Runner::new(bench_suite, opts.clone(), res_tx, pause_rx, cancel.clone());

    let mut collector: Box<dyn ReportCollector> = match cli.collector() {
        Collector::Tui => Box::new(TuiCollector::new(
            opts,
            cli.fps,
            res_rx,
            pause_tx,
            cancel,
            !cli.quit_manually,
        )?),
        Collector::Silent => Box::new(SilentCollector::new(opts, res_rx, cancel)),
    };

    let report = tokio::spawn(async move { collector.run().await });

    runner.run().await?;

    let report = report.await??;

    // Compute comparison using pre-loaded baseline
    let cmp = baseline.map(|b| baseline::compare(&report, &b, cli.noise_threshold, &cli.regression_metrics));

    // Print report with comparison
    let mut output: Box<dyn std::io::Write> = match cli.output_file {
        Some(ref path) => Box::new(File::create(path)?),
        None => Box::new(stdout()),
    };

    match cli.output {
        ReportFormat::Text => TextReporter.print(&mut output, &report, cmp.as_ref())?,
        ReportFormat::Json => JsonReporter.print(&mut output, &report, cmp.as_ref())?,
    }

    // Save baseline if requested (after comparison, so we can compare-then-save)
    if let Some(ref name) = cli.save_baseline {
        baseline::save(&baseline_dir, name, &report, &cli)?;
        println!();
        println!(
            "Baseline '{}' saved to {}",
            name,
            baseline_dir.join(format!("{}.json", name)).display()
        );
    }

    // Handle regression for CI
    if cli.fail_on_regression
        && let Some(ref cmp) = cmp
        && matches!(cmp.verdict, Verdict::Regressed | Verdict::Mixed)
    {
        return Err(RegressionError { verdict: cmp.verdict, baseline: cmp.baseline_name.clone() }.into());
    }

    Ok(())
}

/// A macro to define a CLI struct that embeds `BenchCli`.
#[macro_export]
macro_rules! bench_cli {
    ($name:ident, { $($field:tt)* }) => {
        #[derive(::clap::Parser, Clone)]
        pub struct $name {
            $($field)*

            /// Embed standard BenchCli options into this CLI struct.
            #[command(flatten)]
            pub bench_opts: ::rlt::cli::BenchCli,
        }
    };
}

/// A macro to run the benchmark with the given benchmark suite defined by `bench_cli!`.
/// Note: this requires the $bench_suite to implement `BenchSuite`.
#[macro_export]
macro_rules! bench_cli_run {
    ($bench_suite:ty) => {{
        let b = <$bench_suite>::parse();
        ::rlt::cli::run(b.bench_opts.clone(), b)
    }};
}
