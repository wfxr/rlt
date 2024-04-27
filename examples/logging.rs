use anyhow::Result;
use async_trait::async_trait;
use clap::Parser;
use rlt::{cli::BenchCli, IterInfo, IterReport, StatelessBenchSuite, Status, StatusKind};
use tokio::time::{Duration, Instant};

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Clone)]
struct SimpleBench;

#[async_trait]
impl StatelessBenchSuite for SimpleBench {
    async fn bench(&mut self, info: &IterInfo) -> Result<IterReport> {
        let t = Instant::now();

        // simulate some work
        tokio::time::sleep(Duration::from_micros(info.runner_seq % 30)).await;
        let duration = t.elapsed();

        // simulate status code
        let seq = info.runner_seq;
        let status = match seq % 10 {
            8..=10 => Status::server_error(500),
            6..=7 => Status::client_error(400),
            _ => Status::success(200),
        };

        match status.kind() {
            StatusKind::Success => tracing::info!(?status, seq),
            StatusKind::ClientError => tracing::warn!(?status, seq),
            StatusKind::ServerError | StatusKind::Error => tracing::error!(?status, seq),
        };

        Ok(IterReport { duration, status, bytes: 0, items: 1 })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt = BenchCli::parse();
    match opt.collector() {
        rlt::cli::Collector::Tui => {
            tracing_subscriber::registry()
                .with(EnvFilter::from_default_env())
                .with(rlt::tui_tracing_subscriber_layer())
                .init();
        }
        rlt::cli::Collector::Silent => {
            tracing_subscriber::fmt()
                .with_env_filter(EnvFilter::from_default_env())
                .init();
        }
    }

    rlt::cli::run(opt, SimpleBench).await
}
