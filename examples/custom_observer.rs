use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rlt::observer::Observer;
use rlt::session::BenchSession;
use rlt::{BenchError, BenchOpts, BenchResult, IterInfo, IterReport, StatelessBenchSuite, Status};
use tokio::time;

#[derive(Clone)]
pub struct SimpleBench;

impl StatelessBenchSuite for SimpleBench {
    async fn bench(&mut self, info: &IterInfo) -> BenchResult<IterReport> {
        let t = Instant::now();

        // simulate some work
        time::sleep(Duration::from_micros(info.runner_seq % 30)).await;
        let duration = t.elapsed();

        // simulate status code
        let status = match info.runner_seq % 10 {
            8..=9 => Status::server_error(500),
            6..=7 => Status::client_error(400),
            _ => Status::success(200),
        };

        Ok(IterReport { duration, status, bytes: 1024, items: 1 })
    }
}

#[derive(Debug, Default, Clone)]
struct CountingObserver(Arc<AtomicU64>);

impl Observer for CountingObserver {
    async fn notify(&self, _: Result<&IterReport, &BenchError>) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts = BenchOpts::builder().iterations(1000).build()?;
    let observer = CountingObserver::default();

    let session = BenchSession::new(SimpleBench).opts(opts).observer(observer);
    let report = session.run().await?;
    assert_eq!(report.stats.overall.iters, 1000);
    assert_eq!(report.status_dist[&Status::server_error(500)], 200);
    assert_eq!(report.status_dist[&Status::client_error(400)], 200);
    assert_eq!(report.status_dist[&Status::success(200)], 600);
    Ok(())
}
