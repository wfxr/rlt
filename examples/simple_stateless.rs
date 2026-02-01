use async_trait::async_trait;
use clap::Parser;
use rlt::{BenchResult, IterInfo, IterReport, Result, StatelessBenchSuite, Status, cli::BenchCli};
use tokio::time::{Duration, Instant};

#[derive(Clone)]
struct SimpleBench;

#[async_trait]
impl StatelessBenchSuite for SimpleBench {
    async fn bench(&mut self, info: &IterInfo) -> BenchResult<IterReport> {
        let t = Instant::now();

        // simulate some work
        tokio::time::sleep(Duration::from_micros(info.runner_seq % 30)).await;
        let duration = t.elapsed();

        // simulate status code
        let status = match info.worker_seq % 10 {
            8..=10 => Status::server_error(500),
            6..=7 => Status::client_error(400),
            _ => Status::success(200),
        };

        // simulate items processed in current iteration
        let items = info.worker_seq % 100 + 25;

        Ok(IterReport { duration, status, bytes: items * 1024, items })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    rlt::cli::run(BenchCli::parse(), SimpleBench).await
}
