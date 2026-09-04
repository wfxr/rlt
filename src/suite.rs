//! Module defines trait [`StatelessBenchSuite`] and [`BenchSuite`]. These traits can be used to
//! define benchmarks. The [`BenchSuite::setup`] method is used to setup the benchmark worker and
//! create [`BenchSuite::WorkerState`]. Similarly [`BenchSuite::teardown`] is called at the end of
//! each worker. [`BenchSuite::bench`] and [`StatelessBenchSuite::bench`] are called for each
//! iteration of benchmark.

use std::future;

use crate::{BenchResult, IterInfo, IterReport};

/// A trait for benchmark suites.
pub trait BenchSuite: Sized {
    /// The state for each worker during the benchmark.
    type WorkerState: Send;

    /// Setup procedure before each worker starts.
    /// Initialize and return the worker state (e.g., HTTP client, DB connection).
    fn setup(
        &mut self,
        worker_id: u32,
    ) -> impl Future<Output = BenchResult<Self::WorkerState>> + Send;

    /// Run a single iteration of the benchmark.
    fn bench(
        &mut self,
        state: &mut Self::WorkerState,
        info: &IterInfo,
    ) -> impl Future<Output = BenchResult<IterReport>> + Send;

    /// Teardown procedure after each worker finishes.
    fn teardown(
        self,
        _state: Self::WorkerState,
        _info: IterInfo,
    ) -> impl Future<Output = BenchResult<()>> + Send {
        future::ready(Ok(()))
    }
}

/// A trait for stateless benchmark suites.
pub trait StatelessBenchSuite {
    /// Run a single iteration of the benchmark.
    fn bench(&mut self, info: &IterInfo) -> impl Future<Output = BenchResult<IterReport>> + Send;
}

impl<T> BenchSuite for T
where
    T: StatelessBenchSuite,
{
    type WorkerState = ();

    fn setup(&mut self, _worker_id: u32) -> impl Future<Output = BenchResult<()>> + Send {
        future::ready(Ok(()))
    }

    fn bench(
        &mut self,
        _: &mut Self::WorkerState,
        info: &IterInfo,
    ) -> impl Future<Output = BenchResult<IterReport>> + Send {
        StatelessBenchSuite::bench(self, info)
    }
}
