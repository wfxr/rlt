//! Baseline comparison example.
//!
//! This example demonstrates baseline comparison functionality.
//! Use `--latency` to control the simulated latency, making it easy to
//! observe performance improvements or regressions.
//!
//! ## Quick Start
//!
//! ```bash
//! # 1. Create a baseline
//! cargo run --example baseline -- -c4 -d5s --save-baseline v0
//!
//! # 2. Compare with improved performance (lower latency)
//! cargo run --example baseline -- -c4 -d5s --latency 30ms --baseline v0
//!
//! # 3. Compare with regressed performance (higher latency)
//! cargo run --example baseline -- -c4 -d5s --latency 80ms --baseline v0
//!
//! # 4. Compare and save as new baseline
//! cargo run --example baseline -- -c4 -d5s --latency 40ms --baseline v0 --save-baseline v1
//! ```

use std::time::Duration;

use async_trait::async_trait;
use clap::Parser;
use rlt::{BenchResult, IterInfo, IterReport, Result, StatelessBenchSuite, Status, cli::BenchCli};
use tokio::time::Instant;

#[derive(Parser, Clone)]
struct Opts {
    /// Base latency for simulated work.
    ///
    /// Lower values simulate better performance.
    /// Examples: 100us, 1ms, 500us
    #[clap(long, default_value = "50ms")]
    latency: humantime::Duration,

    #[command(flatten)]
    bench: BenchCli,
}

#[derive(Clone)]
struct BaselineDemo {
    base_latency: Duration,
}

#[async_trait]
impl StatelessBenchSuite for BaselineDemo {
    async fn bench(&mut self, info: &IterInfo) -> BenchResult<IterReport> {
        let t = Instant::now();

        // Simulate work with configurable latency + some variance
        let variance = Duration::from_micros(info.runner_seq % 20);
        let latency = self.base_latency.saturating_add(variance);
        tokio::time::sleep(latency).await;

        let duration = t.elapsed();

        Ok(IterReport {
            duration,
            status: Status::success(200),
            bytes: 1024,
            items: 1,
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();
    let bench = BaselineDemo { base_latency: opts.latency.into() };
    rlt::cli::run(opts.bench, bench).await
}
