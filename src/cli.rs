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
    num::{NonZeroU32, NonZeroU64, NonZeroU8},
    path::PathBuf,
};

use clap::{
    builder::{
        styling::{AnsiColor, Effects},
        Styles,
    },
    Parser, ValueEnum,
};
use crossterm::tty::IsTty;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

use crate::{
    clock::Clock,
    collector::{ReportCollector, SilentCollector, TuiCollector},
    reporter::{BenchReporter, JsonReporter, TextReporter},
    runner::{BenchOpts, BenchSuite, Runner},
};

#[derive(Parser, Clone, Debug)]
#[clap(
    styles(Styles::styled()
        .header(AnsiColor::Yellow.on_default() | Effects::BOLD)
        .usage(AnsiColor::Yellow.on_default() | Effects::BOLD)
        .literal(AnsiColor::Green.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Cyan.on_default())
    )
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

    let reporter: &dyn BenchReporter = match cli.output {
        ReportFormat::Text => &TextReporter,
        ReportFormat::Json => &JsonReporter,
    };

    let report = report.await??;
    match cli.output_file {
        Some(path) => reporter.print(&mut File::create(path)?, &report)?,
        None => reporter.print(&mut stdout(), &report)?,
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
