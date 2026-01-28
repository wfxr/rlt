//! Terminal user interface (TUI) collector for real-time benchmark monitoring.
//!
//! This module provides [`TuiCollector`], an interactive terminal-based collector
//! that displays real-time benchmark statistics with a rich visual interface.
//!
//! # Features
//!
//! - Real-time statistics display (iteration rate, throughput, latency)
//! - Rolling window statistics at multiple time scales (1s, 10s, 1min, 10min)
//! - Latency histogram with percentiles
//! - Iteration histogram showing throughput over time
//! - Status distribution breakdown
//! - Progress bar with duration/iteration tracking
//! - Pause/resume support
//! - Optional log viewer (with `tracing` feature)
//!
//! # Keyboard Controls
//!
//! - `+`/`-`: Zoom time window in/out (switch to manual mode)
//! - `a`: Auto time window (default)
//! - `p`: Pause/resume the benchmark
//! - `l`: Toggle log viewer (requires `tracing` feature)
//! - `q` or `Ctrl+C`: Quit the benchmark

mod input;
mod render;
mod state;
mod terminal;
#[cfg(feature = "tracing")]
mod tui_log;

use anyhow::Result;
use async_trait::async_trait;
use nonzero_ext::nonzero;
use std::{collections::HashMap, num::NonZeroU8, time::Duration};
use tokio::{
    sync::{mpsc, watch},
    time::MissedTickBehavior,
};
use tokio_util::sync::CancellationToken;

use state::{TimeWindowMode, TuiCollectorState};
use terminal::Terminal;

use crate::{
    collector::ReportCollector,
    histogram::LatencyHistogram,
    report::{BenchReport, IterReport},
    runner::BenchOpts,
    stats::{IterStats, RotateDiffWindow, RotateWindowGroup},
    status::Status,
};

const SECOND: Duration = Duration::from_secs(1);

/// A report collector with real-time terminal user interface (TUI) support.
///
/// This collector displays a live dashboard showing benchmark progress,
/// statistics, and histograms. It supports interactive controls for
/// pausing, zooming time windows, and viewing logs.
///
/// The TUI uses [ratatui](https://ratatui.rs) for rendering and updates
/// at the configured frame rate.
///
/// # Display Sections
///
/// - **Stats for last N**: Rolling window statistics (configurable via `+`/`-`)
/// - **Stats overall**: Cumulative statistics since benchmark start
/// - **Status distribution**: Breakdown of response statuses
/// - **Iteration histogram**: Bar chart of iterations per time bucket
/// - **Latency histogram**: Distribution of response latencies with percentiles
/// - **Progress**: Progress bar showing completion status
pub struct TuiCollector {
    /// The benchmark options (duration, iterations, concurrency, etc.).
    pub(crate) bench_opts: BenchOpts,
    /// Refresh rate in frames per second (fps).
    pub(crate) fps: NonZeroU8,
    /// Channel receiver for iteration reports from workers.
    pub(crate) res_rx: mpsc::UnboundedReceiver<Result<IterReport>>,
    /// Watch channel sender for pause/resume control.
    pub(crate) pause: watch::Sender<bool>,
    /// Cancellation token for graceful shutdown.
    pub(crate) cancel: CancellationToken,
    /// Whether to exit automatically when the benchmark finishes.
    pub(crate) auto_quit: bool,

    /// Internal TUI state (time window selection, log state, etc.).
    state: TuiCollectorState,
}

impl TuiCollector {
    /// Create a new TUI report collector.
    pub fn new(
        bench_opts: BenchOpts,
        fps: NonZeroU8,
        res_rx: mpsc::UnboundedReceiver<Result<IterReport>>,
        pause: watch::Sender<bool>,
        cancel: CancellationToken,
        auto_quit: bool,
    ) -> Result<Self> {
        let state = TuiCollectorState {
            tm_win: TimeWindowMode::Auto,
            finished: false,
            #[cfg(feature = "tracing")]
            log: tui_log::LogState::from_env()?,
        };
        Ok(Self { bench_opts, fps, res_rx, pause, cancel, auto_quit, state })
    }
}

#[async_trait]
impl ReportCollector for TuiCollector {
    async fn run(&mut self) -> Result<BenchReport> {
        let mut hist = LatencyHistogram::new();
        let mut stats = IterStats::new();
        let mut status_dist = HashMap::new();
        let mut error_dist = HashMap::new();

        self.collect(&mut hist, &mut stats, &mut status_dist, &mut error_dist)
            .await?;

        let elapsed = self.bench_opts.clock.elapsed();
        let concurrency = self.bench_opts.concurrency;
        Ok(BenchReport { concurrency, hist, stats, status_dist, error_dist, elapsed })
    }
}

impl TuiCollector {
    async fn collect(
        &mut self,
        hist: &mut LatencyHistogram,
        stats: &mut IterStats,
        status_dist: &mut HashMap<Status, u64>,
        error_dist: &mut HashMap<String, u64>,
    ) -> Result<()> {
        let clock = self.bench_opts.clock.clone();
        let mut terminal = Terminal::new()?;

        let mut latest_iters = RotateWindowGroup::new(nonzero!(60usize));
        let mut latest_iters_ticker = clock.ticker(SECOND);

        let mut latest_stats = RotateDiffWindow::new(self.fps.into());
        let mut latest_stats_ticker = clock.ticker(SECOND / self.fps.get() as u32);

        let mut ui_ticker = tokio::time::interval(SECOND / self.fps.get() as u32);
        ui_ticker.set_missed_tick_behavior(MissedTickBehavior::Burst);

        loop {
            if self.state.finished {
                if self.auto_quit {
                    return Ok(());
                }
                ui_ticker.tick().await;
            } else {
                loop {
                    tokio::select! {
                        biased;
                        _ = ui_ticker.tick() => break,
                        _ = latest_stats_ticker.tick() => {
                            latest_stats.rotate(stats.counter);
                            continue;
                        }
                        _ = latest_iters_ticker.tick() => {
                            latest_iters.rotate();
                            continue;
                        }
                        r = self.res_rx.recv() => match r {
                            Some(Ok(report)) => {
                                *status_dist.entry(report.status).or_default() += 1;
                                hist.record(report.duration)?;
                                latest_iters.push(&report);
                                stats.append(&report);
                            }
                            Some(Err(e)) => *error_dist.entry(e.to_string()).or_default() += 1,
                            None => {
                                clock.pause();
                                self.state.finished = true;
                                break;
                            }
                        }
                    };
                }
            }

            let elapsed = clock.elapsed();
            if self.handle_event(elapsed).await? {
                return Ok(());
            }

            terminal.draw(|f| {
                let paused = *self.pause.borrow();
                let tw = self.state.tm_win.effective(elapsed);
                render::render_dashboard(
                    f,
                    &stats.counter,
                    elapsed,
                    &self.bench_opts,
                    paused,
                    self.state.finished,
                    &latest_stats,
                    tw,
                    status_dist,
                    error_dist,
                    &latest_iters,
                    hist,
                );

                #[cfg(feature = "tracing")]
                tui_log::render_logs(f, &self.state.log);
            })?;
        }
    }
}
