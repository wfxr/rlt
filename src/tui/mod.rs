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

use std::collections::HashMap;
use std::num::NonZeroU8;
use std::sync::Arc;
use std::time::Duration;

use futures::channel::mpsc;
use nonzero_ext::nonzero;
use state::{TimeWindow, TimeWindowMode, TuiCollectorState};
use terminal::Terminal;
use tokio::sync::watch;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;

use crate::Error;
use crate::error::TuiError;
use crate::histogram::LatencyHistogram;
use crate::phase::{BenchPhase, PauseControl, RunState};
use crate::report::IterReport;
use crate::runner::BenchOpts;
use crate::stats::{IterStats, MultiScaleStatsWindow, RecentStatsWindow};

type TuiResult<T> = std::result::Result<T, TuiError>;

const SECOND: Duration = Duration::from_secs(1);

/// Settings for Tui
#[derive(Debug, Clone)]
pub struct TuiSettings {
    /// Refresh rate for the tui collector, in frames per second (fps)
    pub fps: NonZeroU8,

    /// Quit automatically once bench is done
    pub auto_quit: bool,
}

impl Default for TuiSettings {
    fn default() -> Self {
        Self { fps: unsafe { NonZeroU8::new_unchecked(32) }, auto_quit: true }
    }
}

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
pub(crate) struct Tui {
    /// The benchmark options (duration, iterations, concurrency, etc.).
    pub(crate) bench_opts: BenchOpts,
    /// Refresh rate in frames per second (fps).
    pub(crate) fps: NonZeroU8,
    /// Channel receiver for iteration reports from workers.
    pub(crate) res_rx: mpsc::UnboundedReceiver<Result<IterReport, String>>,
    /// Pause control shared with the runner.
    pub(crate) pause: Arc<PauseControl>,
    /// Cancellation token for graceful shutdown.
    pub(crate) cancel: CancellationToken,
    /// Whether to exit automatically when the benchmark finishes.
    pub(crate) auto_quit: bool,
    /// Watch channel receiver for benchmark phase status.
    pub(crate) phase_rx: watch::Receiver<BenchPhase>,

    /// Internal TUI state (time window selection, log state, etc.).
    state: TuiCollectorState,
}

impl Tui {
    /// Create a new TUI report collector.
    pub fn new(
        bench_opts: BenchOpts,
        fps: NonZeroU8,
        res_rx: mpsc::UnboundedReceiver<Result<IterReport, String>>,
        pause: Arc<PauseControl>,
        cancel: CancellationToken,
        auto_quit: bool,
        phase_rx: watch::Receiver<BenchPhase>,
    ) -> TuiResult<Self> {
        let state = TuiCollectorState {
            tm_win: TimeWindowMode::Auto,
            run_state: RunState::Running,
            #[cfg(feature = "tracing")]
            log: tui_log::LogState::from_env(),
        };
        Ok(Self { bench_opts, fps, res_rx, pause, cancel, auto_quit, phase_rx, state })
    }
}

impl Tui {
    pub async fn run(mut self) -> Result<(), Error> {
        let mut hist = LatencyHistogram::new();
        let mut stats = IterStats::new();
        let mut status_dist = HashMap::new();
        let mut error_dist = HashMap::new();

        let clock = self.bench_opts.clock.clone();
        let mut terminal = Terminal::new()?;

        let mut latest_iters =
            MultiScaleStatsWindow::new(nonzero!(60usize), TimeWindow::variants().iter().copied())?;
        let mut latest_iters_ticker = clock.ticker(SECOND);

        let mut recent_stats = RecentStatsWindow::new(self.fps.into());
        let mut recent_stats_ticker = clock.ticker(SECOND / self.fps.get() as u32);

        let mut ui_ticker = tokio::time::interval(SECOND / self.fps.get() as u32);
        ui_ticker.set_missed_tick_behavior(MissedTickBehavior::Burst);

        loop {
            if self.state.run_state == RunState::Finished {
                if self.auto_quit {
                    return Ok(());
                }
                ui_ticker.tick().await;
            } else {
                loop {
                    tokio::select! {
                        biased;
                        _ = ui_ticker.tick() => break,
                        _ = recent_stats_ticker.tick() => {
                            recent_stats.record(stats.overall);
                            continue;
                        }
                        _ = latest_iters_ticker.tick() => {
                            latest_iters.tick();
                            continue;
                        }
                        r = self.res_rx.recv() => match r {
                            Ok(Ok(report)) => {
                                *status_dist.entry(report.status).or_default() += 1;
                                hist.record(report.duration)?;
                                latest_iters.push(&report);
                                stats.record(&report);
                            }
                            Ok(Err(e)) => *error_dist.entry(e.to_string()).or_default() += 1,
                            Err(_) => {
                                clock.pause();
                                self.state.run_state = RunState::Finished;
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
                let tw = self.state.tm_win.effective(elapsed);
                let phase = self.phase_rx.borrow().clone();
                render::render_dashboard(
                    f,
                    &stats.overall,
                    elapsed,
                    &self.bench_opts,
                    self.state.run_state,
                    &recent_stats,
                    tw,
                    &status_dist,
                    &error_dist,
                    &latest_iters,
                    &hist,
                    &phase,
                );

                #[cfg(feature = "tracing")]
                tui_log::render_logs(f, &self.state.log);
            })?;
        }
    }
}
