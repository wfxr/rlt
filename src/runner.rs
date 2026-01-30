//! This module defines traits for stateful and stateless benchmark suites.
use anyhow::Result;
use async_trait::async_trait;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    select,
    sync::{Barrier, mpsc, watch},
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

    /// Number of warm-up iterations to run before the main benchmark.
    pub warmups: u64,

    #[cfg(feature = "rate_limit")]
    /// Rate limit for benchmarking, in iterations per second (ips).
    pub rate: Option<NonZeroU32>,
}

/// A trait for benchmark suites.
#[async_trait]
pub trait BenchSuite: Clone {
    /// The state for each worker during the benchmark.
    type WorkerState: Send;

    /// Setup procedure before each worker starts.
    /// Initialize and return the worker state (e.g., HTTP client, DB connection).
    async fn setup(&mut self, worker_id: u32) -> Result<Self::WorkerState>;

    /// Run a single iteration of the benchmark.
    async fn bench(&mut self, state: &mut Self::WorkerState, info: &IterInfo) -> Result<IterReport>;

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

    async fn setup(&mut self, _worker_id: u32) -> Result<()> {
        Ok(())
    }

    async fn bench(&mut self, _: &mut Self::WorkerState, info: &IterInfo) -> Result<IterReport> {
        StatelessBenchSuite::bench(self, info).await
    }
}

/// A Benchmark runner with a given benchmark suite and control options.
#[derive(Clone)]
pub(crate) struct Runner<BS>
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
    BS: BenchSuite + Send + 'static,
    BS::WorkerState: Send + 'static,
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

    async fn iteration(&mut self, state: &mut BS::WorkerState, info: &IterInfo) -> Result<IterReport> {
        self.wait_if_paused().await;
        let res = self.suite.bench(state, info).await;

        #[cfg(feature = "tracing")]
        if let Err(e) = &res {
            log::error!("Error in iteration({info:?}): {:?}", e);
        }

        res
    }

    /// Run the benchmark.
    pub async fn run(self) -> Result<()> {
        let workers = self.opts.concurrency;
        let iters = self.opts.iterations;
        let warmup_iters = self.opts.warmups;

        // the trait `governor::clock::Clock` is not implemented for `&clock::Clock`
        #[cfg(feature = "rate_limit")]
        let buckets = self.opts.rate.map(|r| {
            let quota = Quota::per_second(r).allow_burst(nonzero!(1u32));
            let clock = self.opts.clock.clone();
            Arc::new(RateLimiter::direct_with_clock(quota, clock))
        });

        // Global sequence counter for warmup phase
        let warmup_seq = Arc::new(AtomicU64::new(0));

        // Barrier to synchronize all workers after setup and warmup
        // All workers wait at the barrier, and the clock is started when all are ready
        let barrier = Arc::new(Barrier::new(workers as usize));

        let mut set: JoinSet<Result<()>> = JoinSet::new();
        for worker in 0..workers {
            #[cfg(feature = "rate_limit")]
            let buckets = buckets.clone();
            let mut b = self.clone();
            let warmup_seq = warmup_seq.clone();
            let barrier = barrier.clone();

            set.spawn(async move {
                let mut state = b.suite.setup(worker).await?;
                let mut info = IterInfo::new(worker);
                let cancel = b.cancel.clone();

                // Wait for all workers to complete setup before starting bench loop
                barrier.wait().await;

                // Run warm-up iterations first
                loop {
                    info.runner_seq = warmup_seq.fetch_add(1, Ordering::Relaxed);
                    if info.runner_seq >= warmup_iters {
                        break;
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
                        // Intentionally ignore warm-up results - they are not sent to the result channel
                        _ = b.iteration(&mut state, &info) => (),
                    }
                    info.worker_seq += 1;
                }

                // Wait for all workers to complete setup and warmup before starting main benchmark
                // The leader (last worker to arrive) will start the clock
                if barrier.wait().await.is_leader() {
                    b.opts.clock.resume();
                }

                // Reset worker sequence for main benchmark
                info.worker_seq = 0;

                // Run main benchmark iterations
                loop {
                    info.runner_seq = b.seq.fetch_add(1, Ordering::Relaxed);
                    if let Some(iterations) = iters
                        && info.runner_seq >= iterations
                    {
                        break;
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
                        res = b.iteration(&mut state, &info) => {
                            // safe to ignore the error which means the receiver is dropped
                            let _ = b.res_tx.send(res);
                        },
                    }
                    info.worker_seq += 1;
                }

                // Teardown is called once per worker.
                // Ignore teardown errors as the connection may be in an inconsistent state
                // after cancellation (e.g., timeout). This is expected behavior and should
                // not cause the benchmark to fail.
                if let Err(_e) = b.suite.teardown(state, info).await {
                    #[cfg(feature = "tracing")]
                    log::warn!("Error during teardown for worker {}: {:?}", worker, _e);
                }

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::Instant;

    use crate::{Status, clock::Clock, report::IterReport};

    /// Execution phase for barrier synchronization testing
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Phase {
        Setup,
        Warmup,
        Bench,
    }

    /// A BenchSuite that records phase transitions with timestamps
    #[derive(Clone)]
    struct TrackedSuite {
        events: Arc<Mutex<Vec<(Phase, Instant)>>>,
        setup_delay_ms: u64,
        clock: Clock,
    }

    impl TrackedSuite {
        fn new(setup_delay_ms: u64, clock: Clock) -> Self {
            Self {
                events: Arc::new(Mutex::new(Vec::new())),
                setup_delay_ms,
                clock,
            }
        }

        fn record(&self, phase: Phase) {
            self.events.lock().unwrap().push((phase, Instant::now()));
        }

        fn count(&self, phase: Phase) -> usize {
            self.events.lock().unwrap().iter().filter(|(p, _)| *p == phase).count()
        }

        /// Verify all events of `first` phase complete before any `second` phase event starts
        fn verify_order(&self, first: Phase, second: Phase) -> bool {
            let events = self.events.lock().unwrap();
            let max_first = events.iter().filter(|(p, _)| *p == first).map(|(_, t)| t).max();
            let min_second = events.iter().filter(|(p, _)| *p == second).map(|(_, t)| t).min();
            match (max_first, min_second) {
                (Some(a), Some(b)) => a <= b,
                _ => true,
            }
        }
    }

    #[async_trait]
    impl BenchSuite for TrackedSuite {
        type WorkerState = ();

        async fn setup(&mut self, worker_id: u32) -> Result<()> {
            if worker_id == 0 {
                tokio::time::sleep(Duration::from_millis(self.setup_delay_ms)).await;
            }
            self.record(Phase::Setup);
            Ok(())
        }

        async fn bench(&mut self, _: &mut (), _: &IterInfo) -> Result<IterReport> {
            let phase = if self.clock.elapsed() == Duration::ZERO {
                Phase::Warmup
            } else {
                Phase::Bench
            };
            self.record(phase);
            Ok(IterReport {
                duration: Duration::from_micros(100),
                status: Status::success(200),
                bytes: 0,
                items: 0,
            })
        }
    }

    async fn run_benchmark(suite: &TrackedSuite, concurrency: u32, warmups: u64, iterations: u64) -> Result<()> {
        let (res_tx, mut res_rx) = mpsc::unbounded_channel();
        let (_pause_tx, pause_rx) = watch::channel(false);
        let cancel = CancellationToken::new();

        let opts = BenchOpts {
            clock: suite.clock.clone(),
            concurrency,
            iterations: Some(iterations),
            duration: None,
            warmups,
            #[cfg(feature = "rate_limit")]
            rate: None,
        };

        let runner = Runner::new(suite.clone(), opts, res_tx, pause_rx, cancel);
        let drain = tokio::spawn(async move { while res_rx.recv().await.is_some() {} });

        runner.run().await?;
        drop(drain);
        Ok(())
    }

    #[tokio::test]
    async fn test_setup_barrier_sync() {
        let suite = TrackedSuite::new(50, Clock::new_paused());
        run_benchmark(&suite, 4, 8, 4).await.unwrap();

        assert_eq!(suite.count(Phase::Setup), 4);
        assert_eq!(suite.count(Phase::Warmup), 8);
        assert!(
            suite.verify_order(Phase::Setup, Phase::Warmup),
            "setup should complete before warmup"
        );
    }

    #[tokio::test]
    async fn test_warmup_barrier_sync() {
        let suite = TrackedSuite::new(10, Clock::new_paused());
        run_benchmark(&suite, 4, 8, 8).await.unwrap();

        assert_eq!(suite.count(Phase::Warmup), 8);
        assert_eq!(suite.count(Phase::Bench), 8);
        assert!(
            suite.verify_order(Phase::Warmup, Phase::Bench),
            "warmup should complete before bench"
        );
    }

    #[tokio::test]
    async fn test_clock_starts_after_warmup() {
        let clock = Clock::new_paused();
        let suite = TrackedSuite::new(10, clock.clone());

        assert_eq!(clock.elapsed(), Duration::ZERO);
        run_benchmark(&suite, 2, 4, 4).await.unwrap();

        // Distinct warmup/bench events prove clock was paused during warmup
        assert_eq!(suite.count(Phase::Warmup), 4);
        assert_eq!(suite.count(Phase::Bench), 4);
        assert!(clock.elapsed() > Duration::ZERO);
    }

    #[tokio::test]
    async fn test_no_warmup_still_syncs() {
        let suite = TrackedSuite::new(30, Clock::new_paused());
        run_benchmark(&suite, 3, 0, 6).await.unwrap();

        assert_eq!(suite.count(Phase::Setup), 3);
        assert_eq!(suite.count(Phase::Warmup), 0);
        assert_eq!(suite.count(Phase::Bench), 6);
        assert!(
            suite.verify_order(Phase::Setup, Phase::Bench),
            "setup should complete before bench"
        );
    }
}
