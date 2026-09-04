//! This module defines a [`BenchSession`] that can be used to define a benchmark execution. It runs
//! the benchmark and returns the report or error on completion. It can be optionally configured to
//! use TUI display and also optionally take multiple [`Observer`] to get the results of each
//! iteration

use std::sync::Arc;

use futures::channel::mpsc;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::aggregator::Aggregator;
use crate::observer::{Layered, MpscObserver, Observer, ObserverExt};
use crate::runner::Runner;
use crate::tui::{Tui, TuiSettings};
use crate::{BenchOpts, BenchPhase, BenchReport, BenchSuite, PauseControl};

/// Define a bench session that can be run and get a conslidated report
#[derive(Debug)]
pub struct BenchSession<B, O> {
    suite: B,
    observer: O,
    opts: BenchOpts,
    tui_settings: Option<TuiSettings>,
}

impl<B> BenchSession<B, ()>
where
    B: BenchSuite,
{
    /// Create a new [`BenchSession`] without any custom [`Observer`]s with default [`BenchOpts`]
    /// and disabled TUI interface
    pub fn new(suite: B) -> Self {
        Self { suite, observer: (), opts: BenchOpts::default(), tui_settings: None }
    }

    /// Add an [`Observer`] to the session to get notified for results of each iteration
    pub fn observer<O>(self, observer: O) -> BenchSession<B, O> {
        BenchSession {
            suite: self.suite,
            observer,
            opts: self.opts,
            tui_settings: self.tui_settings,
        }
    }
}

impl<B, O> BenchSession<B, O> {
    /// Update the [`BenchOpts`] to use for the run
    pub fn opts(mut self, opts: BenchOpts) -> Self {
        self.opts = opts;
        self
    }

    /// Enabled TUI with custom settings
    pub fn with_tui(mut self, tui_settings: TuiSettings) -> Self {
        self.tui_settings = Some(tui_settings);
        self
    }

    /// Enabled the TUI with default settings
    pub fn enable_tui(self) -> Self {
        self.with_tui(TuiSettings::default())
    }
}

impl<B, O1> BenchSession<B, O1>
where
    O1: Observer,
{
    /// Add an [`Observer`] to the session to get notified for results of each iteration
    pub fn and_observer<O2>(self, outer: O2) -> BenchSession<B, Layered<O2, O1>>
    where
        O2: Observer,
    {
        BenchSession {
            suite: self.suite,
            observer: self.observer.with(outer),
            opts: self.opts,
            tui_settings: self.tui_settings,
        }
    }
}

impl<B, O> BenchSession<B, O>
where
    B: BenchSuite + Clone + Send + 'static,
    O: Observer + Clone + Send + 'static,
{
    /// Run the benchmark and get the [`BenchReport`] or [BenchError](crate::BenchError)
    pub async fn run(self) -> crate::Result<BenchReport> {
        let Self { suite, observer, opts, tui_settings } = self;

        // Now run the benchmark
        let pause = Arc::new(PauseControl::new());
        let cancel = CancellationToken::new();
        let (phase_tx, phase_rx) =
            watch::channel(BenchPhase::Setup { completed: 0, total: opts.concurrency as usize });

        let (res_tx, res_rx) = mpsc::unbounded();
        let aggregator = tokio::spawn(Aggregator::new(opts.clone(), res_rx, cancel.clone()).run());
        let observer = observer.with(MpscObserver::from(res_tx));

        let (observer, tui) = if let Some(tui_settings) = tui_settings {
            let (res_tx, res_rx) = mpsc::unbounded();
            let tui = Tui::new(
                opts.clone(),
                tui_settings.fps,
                res_rx,
                Arc::clone(&pause),
                cancel.clone(),
                tui_settings.auto_quit,
                phase_rx,
            )?;
            let tui_observer = Some(MpscObserver::from(res_tx));
            let tui_fut = tui.run();
            (observer.with(tui_observer), tokio::spawn(tui_fut))
        } else {
            drop(phase_rx);
            (observer.with(None), tokio::spawn(async move { Ok(()) }))
        };

        let runner = Runner::new(suite, opts, observer, pause, cancel, phase_tx);
        let (bench, tui, report) = futures::join!(runner.run(), tui, aggregator);
        bench?;
        tui??;
        report?
    }
}
