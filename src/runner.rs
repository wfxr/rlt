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
};
use tokio_util::sync::CancellationToken;

cfg_if::cfg_if! {
    if #[cfg(feature = "rate_limit")] {
        use std::num::NonZeroU32;
        use governor::{Quota, RateLimiter};
        use nonzero_ext::nonzero;
    }
}

use crate::{
    clock::Clock,
    // rate_limiter::{self, RateLimiter},
    report::IterReport,
};

/// Core options for the benchmark runner.
#[derive(Clone, Debug)]
pub struct BenchOpts {
    /// Start time of the benchmark.
    pub clock: Clock,

    /// Number of concurrent workers.
    pub concurrency: u32,

    /// Number of iterations to run.
    pub iterations: Option<u64>,

    /// Duration to run the benchmark.
    pub duration: Option<Duration>,

    #[cfg(feature = "rate_limit")]
    /// Rate limit for benchmarking, in iterations per second (ips).
    pub rate: Option<NonZeroU32>,
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

        #[cfg(feature = "log")]
        if let Err(e) = &res {
            log::error!("Error in iteration({info:?}): {:?}", e);
        }
        // safe to ignore the error which means the receiver is dropped
        let _ = self.res_tx.send(res);
    }

    /// Run the benchmark.
    pub async fn run(self) -> Result<()> {
        let concurrency = self.opts.concurrency;
        let iterations = self.opts.iterations;

        #[cfg(feature = "rate_limit")]
        let buckets = self.opts.rate.map(|r| {
            let quota = Quota::per_second(r).allow_burst(nonzero!(1u32));
            let clock = &self.opts.clock;
            Arc::new(RateLimiter::direct_with_clock(quota, clock))
        });

        let mut set: JoinSet<Result<()>> = JoinSet::new();
        for worker in 0..concurrency {
            #[cfg(feature = "rate_limit")]
            let buckets = buckets.clone();
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

                    #[cfg(feature = "rate_limit")]
                    if let Some(buckets) = &buckets {
                        select! {
                            biased;
                            _ = cancel.cancelled() => break,
                            _ = buckets.until_ready() => (),
                        }
                    }

                    select! {
                        biased;
                        _ = cancel.cancelled() => break,
                        _ = b.iteration(&mut state, &info) => (),
                    }
                    info.worker_seq += 1;
                }
                b.suite.teardown(state, info).await?;

                Ok(())
            });
        }

        if let Some(t) = self.opts.duration {
            select! {
                biased;
                _ = self.cancel.cancelled() => (),
                _ = self.opts.clock.sleep(t) => self.cancel.cancel(),
                _ = join_all(&mut set) => (),
            }
        };

        join_all(&mut set).await
    }

    async fn wait_if_paused(&mut self) {
        while *self.pause.borrow() {
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
