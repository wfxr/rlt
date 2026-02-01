//! This module defines traits for stateful and stateless benchmark suites.
use async_trait::async_trait;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
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
    error::{BenchResult, ConfigError, Error, Result},
    phase::{BenchPhase, PauseControl},
    report::IterReport,
};

/// Core options for the benchmark runner.
#[derive(Clone, Debug)]
pub struct BenchOpts {
    /// Benchmark clock used for measuring elapsed time and driving tickers.
    ///
    /// The runner typically starts this clock paused and resumes it when entering the main
    /// [`BenchPhase::Bench`] phase, so setup/warmup time is excluded from the reported elapsed time.
    /// It can be paused/resumed (e.g. by the TUI) to implement runtime pause.
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

impl Default for BenchOpts {
    fn default() -> Self {
        Self {
            clock: Clock::new_paused(),
            concurrency: 1,
            iterations: None,
            duration: None,
            warmups: 0,
            #[cfg(feature = "rate_limit")]
            rate: None,
        }
    }
}

impl BenchOpts {
    /// Create a builder for [`BenchOpts`].
    ///
    /// This provides ergonomic construction without having to spell out all fields,
    /// and it keeps feature-gated setters (e.g. `rate`) available even when the
    /// corresponding feature is disabled.
    ///
    /// When the `rate_limit` feature is disabled, calling [`BenchOptsBuilder::rate`]
    /// will cause [`BenchOptsBuilder::build`] to return an error.
    pub fn builder() -> BenchOptsBuilder {
        BenchOptsBuilder::default()
    }
}

/// Builder for [`BenchOpts`].
#[derive(Clone, Debug)]
pub struct BenchOptsBuilder {
    clock: Clock,
    concurrency: u32,
    iterations: Option<u64>,
    duration: Option<Duration>,
    warmups: u64,
    rate: Option<u32>,
}

impl Default for BenchOptsBuilder {
    fn default() -> Self {
        Self {
            clock: Clock::new_paused(),
            concurrency: 1,
            iterations: None,
            duration: None,
            warmups: 0,
            rate: None,
        }
    }
}

impl BenchOptsBuilder {
    /// Set the benchmark clock.
    pub fn clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        self
    }

    /// Set the number of concurrent workers.
    pub fn concurrency(mut self, n: u32) -> Self {
        self.concurrency = n;
        self
    }

    /// Stop after running the given number of iterations.
    pub fn iterations(mut self, n: u64) -> Self {
        self.iterations = Some(n);
        self
    }

    /// Stop after running for the given duration.
    pub fn duration(mut self, d: Duration) -> Self {
        self.duration = Some(d);
        self
    }

    /// Set the number of warm-up iterations.
    pub fn warmups(mut self, n: u64) -> Self {
        self.warmups = n;
        self
    }

    /// Set the rate limit in iterations per second (ips).
    ///
    /// When the `rate_limit` feature is disabled, [`build`](Self::build) will return an error.
    pub fn rate(mut self, r: u32) -> Self {
        self.rate = Some(r);
        self
    }

    /// Build [`BenchOpts`].
    pub fn build(self) -> std::result::Result<BenchOpts, ConfigError> {
        if self.concurrency == 0 {
            return Err(ConfigError::ConcurrencyZero);
        }

        #[cfg(feature = "rate_limit")]
        let rate = match self.rate {
            None => None,
            Some(r) => {
                let r = NonZeroU32::new(r).ok_or(ConfigError::RateZero)?;
                Some(r)
            }
        };

        #[cfg(not(feature = "rate_limit"))]
        if self.rate.is_some() {
            return Err(ConfigError::RateLimitFeatureDisabled);
        }

        Ok(BenchOpts {
            clock: self.clock,
            concurrency: self.concurrency,
            iterations: self.iterations,
            duration: self.duration,
            warmups: self.warmups,
            #[cfg(feature = "rate_limit")]
            rate,
        })
    }
}

/// A trait for benchmark suites.
#[async_trait]
pub trait BenchSuite: Clone {
    /// The state for each worker during the benchmark.
    type WorkerState: Send;

    /// Setup procedure before each worker starts.
    /// Initialize and return the worker state (e.g., HTTP client, DB connection).
    async fn setup(&mut self, worker_id: u32) -> BenchResult<Self::WorkerState>;

    /// Run a single iteration of the benchmark.
    async fn bench(&mut self, state: &mut Self::WorkerState, info: &IterInfo) -> BenchResult<IterReport>;

    /// Teardown procedure after each worker finishes.
    #[allow(unused_variables)]
    async fn teardown(self, state: Self::WorkerState, info: IterInfo) -> BenchResult<()> {
        Ok(())
    }
}

/// A trait for stateless benchmark suites.
#[async_trait]
pub trait StatelessBenchSuite {
    /// Run a single iteration of the benchmark.
    async fn bench(&mut self, info: &IterInfo) -> BenchResult<IterReport>;
}

#[async_trait]
impl<T> BenchSuite for T
where
    T: StatelessBenchSuite + Clone + Send + Sync + 'static,
{
    type WorkerState = ();

    async fn setup(&mut self, _worker_id: u32) -> BenchResult<()> {
        Ok(())
    }

    async fn bench(&mut self, _: &mut Self::WorkerState, info: &IterInfo) -> BenchResult<IterReport> {
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
    res_tx: mpsc::UnboundedSender<BenchResult<IterReport>>,
    pause: Arc<PauseControl>,
    cancel: CancellationToken,
    seq: Arc<AtomicU64>,
    phase_tx: watch::Sender<BenchPhase>,
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
        res_tx: mpsc::UnboundedSender<BenchResult<IterReport>>,
        pause: Arc<PauseControl>,
        cancel: CancellationToken,
        phase_tx: watch::Sender<BenchPhase>,
    ) -> Self {
        Self {
            suite,
            opts,
            res_tx,
            pause,
            cancel,
            seq: Arc::default(),
            phase_tx,
        }
    }

    async fn iteration(&mut self, state: &mut BS::WorkerState, info: &IterInfo) -> BenchResult<IterReport> {
        self.pause.wait_if_paused().await;
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

        // Counter to track warmup completion progress.
        //
        // NOTE: Warmup runs concurrently across workers, so we must count completions
        // rather than using the issued global sequence number (completions may finish
        // out-of-order).
        let warmup_completed = Arc::new(AtomicU64::new(0));

        // Counter to track setup completion progress
        let setup_completed = Arc::new(AtomicUsize::new(0));

        // Barrier to synchronize all workers after setup and warmup
        // All workers wait at the barrier, and the clock is started when all are ready
        let barrier = Arc::new(Barrier::new(workers as usize));

        let mut set: JoinSet<Result<()>> = JoinSet::new();
        for worker in 0..workers {
            #[cfg(feature = "rate_limit")]
            let buckets = buckets.clone();
            let mut b = self.clone();
            let warmup_seq = warmup_seq.clone();
            let warmup_completed = warmup_completed.clone();
            let setup_completed = setup_completed.clone();
            let barrier = barrier.clone();

            set.spawn(async move {
                let mut state = b
                    .suite
                    .setup(worker)
                    .await
                    .map_err(|e| Error::WorkerSetup { worker_id: worker, source: e })?;
                let mut info = IterInfo::new(worker);
                let cancel = b.cancel.clone();

                // Report setup progress
                let completed = setup_completed.fetch_add(1, Ordering::Relaxed) + 1;
                // Progress display must be best-effort; never fail the benchmark if receivers are gone.
                b.phase_tx
                    .send_replace(BenchPhase::Setup { completed, total: workers as usize });

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
                        _ = b.iteration(&mut state, &info) => {
                            info.worker_seq += 1;

                            // Report warmup progress after an iteration completes.
                            let completed = warmup_completed.fetch_add(1, Ordering::Relaxed) + 1;
                            b.phase_tx.send_replace(BenchPhase::Warmup { completed, total: warmup_iters });
                        },
                    }
                }

                // Wait for all workers to complete setup and warmup before starting main benchmark
                // The leader (last worker to arrive) will start the clock
                if barrier.wait().await.is_leader() {
                    b.phase_tx.send_replace(BenchPhase::Bench);

                    // Only start the clock once the benchmark phase begins AND we're not paused.
                    // When paused, the clock must not advance (e.g. duration mode should not count down).
                    if !b.pause.is_paused() {
                        b.opts.clock.resume();
                    }
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
    use std::env;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    use std::time::Instant;

    use crate::{Status, clock::Clock, report::IterReport};

    #[test]
    fn test_bench_opts_default_values() {
        let opts = BenchOpts::default();
        assert_eq!(opts.concurrency, 1);
        assert_eq!(opts.iterations, None);
        assert_eq!(opts.duration, None);
        assert_eq!(opts.warmups, 0);
    }

    #[test]
    fn test_bench_opts_builder_sets_fields() {
        let opts = BenchOpts::builder()
            .concurrency(4)
            .iterations(1000)
            .duration(Duration::from_secs(3))
            .warmups(10)
            .build()
            .unwrap();

        assert_eq!(opts.concurrency, 4);
        assert_eq!(opts.iterations, Some(1000));
        assert_eq!(opts.duration, Some(Duration::from_secs(3)));
        assert_eq!(opts.warmups, 10);
    }

    #[cfg(feature = "rate_limit")]
    #[test]
    fn test_bench_opts_builder_rate_conversion() {
        let opts = BenchOpts::builder().rate(100).build().unwrap();
        assert_eq!(opts.rate, NonZeroU32::new(100));
    }

    #[cfg(feature = "rate_limit")]
    #[test]
    fn test_bench_opts_builder_rate_zero_rejected() {
        let err = BenchOpts::builder().rate(0).build().unwrap_err();
        assert!(err.to_string().contains("rate must be non-zero"));
    }

    #[cfg(not(feature = "rate_limit"))]
    #[test]
    fn test_bench_opts_builder_rate_requires_feature() {
        let err = BenchOpts::builder().rate(100).build().unwrap_err();
        assert!(err.to_string().contains("feature `rate_limit` is disabled"));
    }

    #[test]
    #[ignore]
    fn perf_pause_hotpath_throughput() {
        #[derive(Clone)]
        struct EmptyBench;

        #[async_trait]
        impl StatelessBenchSuite for EmptyBench {
            async fn bench(&mut self, _: &IterInfo) -> BenchResult<IterReport> {
                Ok(IterReport {
                    duration: Duration::ZERO,
                    status: Status::success(0),
                    bytes: 0,
                    items: 0,
                })
            }
        }

        let concurrency: u32 = env::var("RLT_PERF_CONCURRENCY")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(32);

        let iterations: u64 = env::var("RLT_PERF_ITERS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(20_000_000);

        let threads: usize = env::var("RLT_PERF_THREADS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1));

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_time()
            .worker_threads(threads)
            .build()
            .unwrap();

        rt.block_on(async move {
            let (res_tx, res_rx) = mpsc::unbounded_channel::<BenchResult<IterReport>>();
            drop(res_rx);

            let pause = Arc::new(PauseControl::new());
            let (phase_tx, phase_rx) = watch::channel(BenchPhase::default());
            drop(phase_rx);

            let cancel = CancellationToken::new();
            let opts = BenchOpts::builder()
                .clock(Clock::new_paused())
                .concurrency(concurrency)
                .iterations(iterations)
                .warmups(0)
                .build()
                .unwrap();

            let runner = Runner::new(EmptyBench, opts, res_tx, pause, cancel, phase_tx);

            let t0 = Instant::now();
            runner.run().await.unwrap();
            let elapsed = t0.elapsed();

            let iters_per_sec = iterations as f64 / elapsed.as_secs_f64();
            eprintln!(
                "perf_pause_hotpath_throughput pause_impl=pause_control concurrency={} iterations={} threads={} elapsed={:?} iters/sec={:.0}",
                concurrency,
                iterations,
                threads,
                elapsed,
                iters_per_sec
            );
        });
    }

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

        async fn setup(&mut self, worker_id: u32) -> BenchResult<()> {
            if worker_id == 0 {
                tokio::time::sleep(Duration::from_millis(self.setup_delay_ms)).await;
            }
            self.record(Phase::Setup);
            Ok(())
        }

        async fn bench(&mut self, _: &mut (), _: &IterInfo) -> BenchResult<IterReport> {
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
        let pause = Arc::new(PauseControl::new());
        let (phase_tx, _phase_rx) = watch::channel(BenchPhase::default());
        let cancel = CancellationToken::new();

        let opts = BenchOpts::builder()
            .clock(suite.clock.clone())
            .concurrency(concurrency)
            .iterations(iterations)
            .warmups(warmups)
            .build()?;

        let runner = Runner::new(suite.clone(), opts, res_tx, pause, cancel, phase_tx);
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

    #[tokio::test]
    async fn test_phase_updates_are_best_effort_when_receiver_is_dropped() {
        let suite = TrackedSuite::new(0, Clock::new_paused());
        let (res_tx, mut res_rx) = mpsc::unbounded_channel();
        let pause = Arc::new(PauseControl::new());
        let (phase_tx, phase_rx) = watch::channel(BenchPhase::default());
        drop(phase_rx);
        let cancel = CancellationToken::new();

        let opts = BenchOpts::builder()
            .clock(suite.clock.clone())
            .concurrency(2)
            .iterations(1)
            .warmups(1)
            .build()
            .unwrap();

        let runner = Runner::new(suite.clone(), opts, res_tx, pause, cancel, phase_tx);
        let drain = tokio::spawn(async move { while res_rx.recv().await.is_some() {} });

        runner.run().await.unwrap();
        drop(drain);
    }

    #[derive(Clone)]
    struct PauseAtBenchSuite {
        warmups: u64,
        // Signals once when the global "last warmup iteration" starts running.
        last_warmup_started: Arc<tokio::sync::Notify>,
        last_warmup_seen: Arc<AtomicBool>,
        delay: Duration,
    }

    #[async_trait]
    impl BenchSuite for PauseAtBenchSuite {
        type WorkerState = ();

        async fn setup(&mut self, _worker_id: u32) -> BenchResult<()> {
            Ok(())
        }

        async fn bench(&mut self, _: &mut (), info: &IterInfo) -> BenchResult<IterReport> {
            if self.warmups > 0
                && info.runner_seq + 1 == self.warmups
                && !self.last_warmup_seen.swap(true, Ordering::Relaxed)
            {
                self.last_warmup_started.notify_one();
                tokio::time::sleep(self.delay).await;
            }

            Ok(IterReport {
                duration: Duration::from_micros(100),
                status: Status::success(200),
                bytes: 0,
                items: 0,
            })
        }
    }

    #[tokio::test]
    async fn test_paused_does_not_start_clock_at_bench_transition() {
        let clock = Clock::new_paused();
        let warmups = 8;
        let last_warmup_started = Arc::new(tokio::sync::Notify::new());

        let suite = PauseAtBenchSuite {
            warmups,
            last_warmup_started: last_warmup_started.clone(),
            last_warmup_seen: Arc::new(AtomicBool::new(false)),
            delay: Duration::from_millis(50),
        };

        let (res_tx, mut res_rx) = mpsc::unbounded_channel();
        let pause = Arc::new(PauseControl::new());
        let (phase_tx, mut phase_rx) = watch::channel(BenchPhase::default());
        let cancel = CancellationToken::new();

        let opts = BenchOpts::builder()
            .clock(clock.clone())
            .concurrency(2)
            .iterations(2)
            .warmups(warmups)
            .build()
            .unwrap();

        let runner = Runner::new(suite, opts, res_tx, pause.clone(), cancel, phase_tx);
        let drain = tokio::spawn(async move { while res_rx.recv().await.is_some() {} });
        let handle = tokio::spawn(async move { runner.run().await });

        // Pause while the last warmup iteration is still running, so the transition to Bench happens under Paused.
        last_warmup_started.notified().await;
        pause.pause();

        // Wait for the Bench transition.
        loop {
            if matches!(&*phase_rx.borrow(), BenchPhase::Bench) {
                break;
            }
            phase_rx.changed().await.unwrap();
        }

        assert_eq!(clock.elapsed(), Duration::ZERO);

        // Simulate TUI: resume clock + unpause once bench phase is reached.
        clock.resume();
        pause.resume();

        handle.await.unwrap().unwrap();
        drop(drain);
    }
}
