use std::sync::{atomic::AtomicU64, Arc};

use anyhow::Result;
use async_trait::async_trait;
use clap::Parser;
use rlt::{
    cli::BenchCli,
    IterReport, Status, {IterInfo, StatelessBenchSuite},
};
use tokio::time::{Duration, Instant};

/// Demonstration of warmup functionality.
///
/// This example simulates a service that has:
/// - Cold start: First 10 iterations take 100-200ms (simulating initialization, JIT warmup, cache misses, etc.)
/// - Warm state: Subsequent iterations take 1-5ms (simulating optimized performance)
///
/// The warmup phase "warms up" the system (simulated via shared state), and then
/// the main benchmark benefits from this warmed-up state.
///
/// Run with warmup to exclude the slow cold start from measurements:
/// ```
/// cargo run --example warmup -- -w 10 -n 20
/// ```
///
/// Compare with no warmup to see the impact:
/// ```
/// cargo run --example warmup -- -n 30
/// ```
#[derive(Clone)]
struct SimpleBench {
    /// Track total iterations across all phases (warmup + main)
    /// This simulates a system that gets progressively warmer
    iterations: Arc<AtomicU64>,
}

#[async_trait]
impl StatelessBenchSuite for SimpleBench {
    async fn bench(&mut self, info: &IterInfo) -> Result<IterReport> {
        let t = Instant::now();

        // Track total system iterations across all phases (warmup + main)
        let total_iter = self.iterations.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Simulate cold start for first 10 total iterations (across all workers and phases)
        // After that, simulate warm performance
        let sleep_duration = if total_iter < 10 {
            // Cold start: 100-200ms with some variation
            Duration::from_millis(100 + (total_iter % 10) * 10)
        } else {
            // Warm state: 1-5ms with minimal variation
            Duration::from_millis(1 + (total_iter % 5))
        };

        tokio::time::sleep(sleep_duration).await;
        let duration = t.elapsed();

        // Simulate varying work items processed
        let items = if total_iter < 10 {
            // Cold start processes fewer items
            (info.worker_seq % 10) + 1
        } else {
            // Warm state processes more items efficiently
            (info.worker_seq % 50) + 25
        };

        let status = Status::success(200);
        Ok(IterReport { duration, status, bytes: items * 1024, items })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    rlt::cli::run(BenchCli::parse(), SimpleBench { iterations: Arc::default() }).await
}
