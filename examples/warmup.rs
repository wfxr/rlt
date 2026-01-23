use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use anyhow::Result;
use async_trait::async_trait;
use clap::Parser;
use rlt::{
    IterReport, Status,
    cli::BenchCli,
    {IterInfo, StatelessBenchSuite},
};
use tokio::time::{Duration, Instant};

/// Demonstration of warmup functionality.
///
/// This example simulates a service that has:
/// - Cold start: First 10 iterations take 100-200ms (simulating initialization, JIT warmup, cache
///   misses, etc.)
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
    iters: Arc<AtomicU64>,
}

const WARMUP_ITERS: u64 = 10;

#[async_trait]
impl StatelessBenchSuite for SimpleBench {
    async fn bench(&mut self, info: &IterInfo) -> Result<IterReport> {
        let t = Instant::now();

        let d = if self.iters.fetch_add(1, Ordering::Relaxed) < WARMUP_ITERS {
            // Cold start: 100-200ms with some variation
            Duration::from_millis(100 + (info.worker_seq % WARMUP_ITERS) * 10)
        } else {
            // Warm state: 1-5ms with minimal variation
            Duration::from_millis(1 + (info.worker_seq % 5))
        };
        tokio::time::sleep(d).await;

        let duration = t.elapsed();
        let items = info.worker_seq % 100 + 25;
        let status = Status::success(200);
        Ok(IterReport { duration, status, bytes: items * 1024, items })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    rlt::cli::run(BenchCli::parse(), SimpleBench { iters: Arc::default() }).await
}
