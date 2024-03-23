use std::io::stdout;

use clap::{Parser, ValueEnum};
use crossterm::tty::IsTty;
use tokio::{
    sync::{mpsc, watch},
    time::Instant,
};
use tokio_util::sync::CancellationToken;

use crate::{
    collector::{ReportCollector, SilentCollector, TuiCollector},
    reporter::{BenchReporter, JsonReporter, TextReporter},
    runner::{BenchOpts, BenchSuite, Runner},
};

#[derive(Parser, Clone)]
pub struct BenchCli {
    /// Number of workers to run concurrently
    #[clap(long, short = 'c', default_value = "1", value_parser = clap::value_parser!(u32).range(1..))]
    pub concurrency: u32,

    /// Number of iterations
    ///
    /// When set, benchmark stops after reaching the number of iterations.
    #[clap(long, short = 'n', value_parser = clap::value_parser!(u64).range(1..))]
    pub iterations: Option<u64>,

    /// Duration to run the benchmark
    ///
    /// When set, benchmark stops after reaching the duration.
    ///
    /// Examples: -z 10s, -z 5m, -z 1h
    #[clap(long, short = 'd')]
    pub duration: Option<humantime::Duration>,

    /// Rate limit for benchmarking, in iterations per second (ips)
    ///
    /// When set, benchmark will try to run at the specified rate.
    #[clap(long, short = 'r', value_parser = clap::value_parser!(u32).range(1..))]
    pub rate: Option<u32>,

    /// Run benchmark in quiet mode
    ///
    /// Implies --collector silent.
    #[clap(long, short = 'q')]
    pub quiet: bool,

    /// Collector for the benchmark
    #[clap(long, value_enum, ignore_case = true)]
    pub collector: Option<Collector>,

    /// Refresh rate for the tui collector, in frames per second (fps)
    #[clap(long, default_value = "32", value_parser = clap::value_parser!(u8).range(1..))]
    pub fps: u8,

    /// Output format for the report
    #[clap(short, long, value_enum, default_value_t = ReportFormat::Text, ignore_case = true)]
    pub output: ReportFormat,
}

impl BenchCli {
    pub fn bench_opts(&self, start: Instant) -> BenchOpts {
        BenchOpts {
            start,
            concurrency: self.concurrency,
            iterations: self.iterations,
            duration: self.duration.map(|d| d.into()),
            rate: self.rate,
        }
    }

    pub fn collector(&self) -> Collector {
        match self.collector {
            Some(collector) => collector,
            None if self.quiet || !stdout().is_tty() => Collector::Silent,
            _ => Collector::Tui,
        }
    }
}

#[derive(Copy, Clone, ValueEnum)]
pub enum Collector {
    Tui,
    Silent,
}

#[derive(Copy, Clone, ValueEnum)]
pub enum ReportFormat {
    Text,
    Json,
}

pub async fn run<BS>(cli: &BenchCli, bench_suite: BS) -> anyhow::Result<()>
where
    BS: BenchSuite + Send + Sync + 'static,
    BS::RunnerState: Send + Sync + 'static,
{
    let (res_tx, res_rx) = mpsc::unbounded_channel();
    let (pause_tx, pause_rx) = watch::channel(false);
    let cancel = CancellationToken::new();

    let opts = cli.bench_opts(Instant::now());
    let runner = Runner::new(bench_suite, opts, res_tx, pause_rx, cancel.clone());

    let mut collector: Box<dyn ReportCollector> = match cli.collector() {
        Collector::Tui => Box::new(TuiCollector::new(opts, cli.fps, res_rx, pause_tx, cancel)),
        Collector::Silent => Box::new(SilentCollector::new(opts, res_rx, cancel)),
    };

    let report = tokio::spawn(async move { collector.run().await });

    runner.run().await?;

    let reporter: &dyn BenchReporter = match cli.output {
        ReportFormat::Text => &TextReporter,
        ReportFormat::Json => &JsonReporter,
    };

    reporter.print(&mut stdout(), &report.await??)?;

    Ok(())
}
