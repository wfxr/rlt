//! This module defines traits for stateful and stateless benchmark suites.
use anyhow::Result;
use async_trait::async_trait;
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    select,
    sync::{mpsc, watch},
    task::JoinSet,
    time::{sleep_until, Instant, MissedTickBehavior},
};
use tokio_util::sync::CancellationToken;

use crate::report::IterReport;

/// Core options for the benchmark runner.
#[derive(Copy, Clone, Debug)]
pub struct BenchOpts {
    /// Start time of the benchmark.
    pub start: Instant,

    /// Number of concurrent workers.
    pub concurrency: u32,

    /// Number of iterations to run.
    pub iterations: Option<u64>,

    /// Duration to run the benchmark.
    pub duration: Option<Duration>,

    /// Rate limit for benchmarking, in iterations per second (ips).
    pub rate: Option<u32>,
}

impl BenchOpts {
    pub(crate) fn endtime(&self) -> Option<Instant> {
        self.duration.map(|d| self.start + d)
    }
}

/// A trait for benchmark suites.
#[async_trait]
pub trait BenchSuite: Clone {
    /// The state for each worker during the benchmark.
    type WorkerState: Send;

    /// Initialize the state for a worker.
    async fn state(&self, worker_id: u32) -> Result<Self::WorkerState>;

    /// Run a single iteration of the benchmark.
    async fn bench(&mut self, state: &mut Self::WorkerState, info: &IterInfo) -> Result<IterReport>;

    /// Setup procedure before each worker starts.
    #[allow(unused_variables)]
    async fn setup(&mut self, state: &mut Self::WorkerState, worker_id: u32) -> Result<()> {
        Ok(())
    }

    /// Teardown procedure after each worker finishes.
    #[allow(unused_variables)]
    async fn teardown(self, state: Self::WorkerState, info: IterInfo) -> Result<()> {
        Ok(())
    }
}

/// A trait for stateless benchmark suites.
#[async_trait]
pub trait StatelessBenchSuite {
    /// Run a single iteration of the benchmark.
    async fn bench(&mut self, info: &IterInfo) -> Result<IterReport>;
}

#[async_trait]
impl<T> BenchSuite for T
where
    T: StatelessBenchSuite + Clone + Send + Sync + 'static,
{
    type WorkerState = ();

    async fn state(&self, _: u32) -> Result<()> {
        Ok(())
    }

    async fn bench(&mut self, _: &mut Self::WorkerState, info: &IterInfo) -> Result<IterReport> {
        StatelessBenchSuite::bench(self, info).await
    }
}

/// A Benchmark runner with a given benchmark suite and control options.
#[derive(Clone)]
pub struct Runner<BS>
where
    BS: BenchSuite,
{
    suite: BS,
    opts: BenchOpts,
    res_tx: mpsc::UnboundedSender<Result<IterReport>>,
    pause: watch::Receiver<bool>,
    cancel: CancellationToken,
    seq: Arc<AtomicU64>,
}

/// Information about the current iteration.
#[derive(Debug, Clone)]
pub struct IterInfo {
    /// The id of the current worker.
    pub worker_id: u32,

    /// The iteration sequence number of the current worker.
    pub worker_seq: u64,

    /// The iteration sequence number of the current runner.
    pub runner_seq: u64,
}

impl IterInfo {
    /// Create a new iteration info for the given worker id.
    pub fn new(worker_id: u32) -> Self {
        Self { worker_id, worker_seq: 0, runner_seq: 0 }
    }
}

impl<BS> Runner<BS>
where
    BS: BenchSuite + Send + Sync + 'static,
    BS::WorkerState: Send + Sync + 'static,
{
    /// Create a new benchmark runner with the given benchmark suite and options.
    pub fn new(
        suite: BS,
        opts: BenchOpts,
        res_tx: mpsc::UnboundedSender<Result<IterReport>>,
        pause: watch::Receiver<bool>,
        cancel: CancellationToken,
    ) -> Self {
        Self { suite, opts, res_tx, pause, cancel, seq: Arc::default() }
    }

    async fn iteration(&mut self, state: &mut BS::WorkerState, info: &IterInfo) {
        self.wait_if_paused().await;
        let res = self.suite.bench(state, info).await;
        if let Err(e) = &res {
            log::error!("Error in iteration({info:?}): {:?}", e);
        }
        self.res_tx.send(res).expect("send report");
    }

    /// Run the benchmark.
    pub async fn run(self) -> Result<()> {
        match self.opts.rate {
            None => self.bench().await,
            Some(r) => self.bench_with_rate(r).await,
        }
    }

    /// Run the benchmark without a rate limit.
    async fn bench(self) -> Result<()> {
        let concurrency = self.opts.concurrency;
        let iterations = self.opts.iterations;
        let endtime = self.opts.endtime();

        let mut set: JoinSet<Result<()>> = JoinSet::new();
        for worker in 0..concurrency {
            let mut b = self.clone();
            set.spawn(async move {
                let mut state = b.suite.state(worker).await?;
                let mut info = IterInfo::new(worker);
                let cancel = b.cancel.clone();

                b.suite.setup(&mut state, worker).await?;
                loop {
                    info.runner_seq = b.seq.fetch_add(1, Ordering::Relaxed);
                    if let Some(iterations) = iterations {
                        if info.runner_seq >= iterations {
                            break;
                        }
                    }
                    select! {
                        _ = cancel.cancelled() => break,
                        _ = b.iteration(&mut state, &info) => (),
                    }
                    info.worker_seq += 1;
                }
                b.suite.teardown(state, info).await?;

                Ok(())
            });
        }

        if let Some(t) = endtime {
            select! {
                _ = self.cancel.cancelled() => (),
                _ = sleep_until(t) => self.cancel.cancel(),
                _ = join_all(&mut set) => (),
            }
        };

        join_all(&mut set).await
    }

    /// Run the benchmark with a rate limit.
    async fn bench_with_rate(self, rate: u32) -> Result<()> {
        let concurrency = self.opts.concurrency;
        let iterations = self.opts.iterations;
        let endtime = self.opts.endtime();
        let (tx, rx) = flume::bounded(self.opts.concurrency as usize);

        let b = self.clone();
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(1) / rate);
            timer.set_missed_tick_behavior(MissedTickBehavior::Burst);
            let mut iter = 0;
            loop {
                let t = timer.tick().await;
                if b.paused() {
                    match b.cancel.is_cancelled() {
                        false => continue,
                        true => break,
                    }
                }
                if matches!(endtime, Some(endtime) if t >= endtime) {
                    break;
                }
                if matches!(iterations, Some(iterations) if iter >= iterations) {
                    break;
                }
                if tx.send_async(()).await.is_err() {
                    // receiver dropped
                    break;
                }
                iter += 1;
            }
        });

        let mut set: JoinSet<Result<()>> = JoinSet::new();
        for worker in 0..concurrency {
            let mut b = self.clone();
            let rx = rx.clone();
            set.spawn(async move {
                let mut state = b.suite.state(worker).await?;
                let mut info = IterInfo::new(worker);
                let cancel = b.cancel.clone();

                b.suite.setup(&mut state, worker).await?;
                loop {
                    select! {
                        _ = cancel.cancelled() => break,
                        t = rx.recv_async() => match t {
                            Ok(_) => {
                                info.runner_seq = b.seq.fetch_add(1, Ordering::Relaxed);
                                select! {
                                    _ = cancel.cancelled() => break,
                                    _ = b.iteration(&mut state, &info) => (),
                                }
                                info.worker_seq += 1;
                            }
                            Err(_) => break,
                        }
                    }
                }
                b.suite.teardown(state, info).await?;

                Ok(())
            });
        }

        join_all(&mut set).await
    }

    fn paused(&self) -> bool {
        *self.pause.borrow()
    }

    async fn wait_if_paused(&mut self) {
        while self.paused() {
            if self.pause.changed().await.is_err() {
                return;
            }
        }
    }
}

async fn join_all(set: &mut JoinSet<Result<()>>) -> Result<()> {
    while let Some(res) = set.join_next().await {
        res??;
    }
    Ok(())
}
