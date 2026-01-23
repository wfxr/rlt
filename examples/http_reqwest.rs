use anyhow::Result;
use async_trait::async_trait;
use clap::Parser;
use reqwest::{Client, Url};
use rlt::{BenchSuite, IterInfo, IterReport, bench_cli, bench_cli_run};
use tokio::time::Instant;

bench_cli!(HttpBench, {
    /// Target URL.
    #[clap(long)]
    pub url: Url,
});

#[async_trait]
impl BenchSuite for HttpBench {
    type WorkerState = Client;

    async fn state(&self, _: u32) -> Result<Self::WorkerState> {
        Ok(Client::new())
    }

    async fn bench(&mut self, client: &mut Self::WorkerState, _: &IterInfo) -> Result<IterReport> {
        let t = Instant::now();
        let resp = client.get(self.url.clone()).send().await?;
        let status = resp.status().into();
        let bytes = resp.bytes().await?.len() as u64;
        let duration = t.elapsed();
        Ok(IterReport { duration, status, bytes, items: 1 })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    bench_cli_run!(HttpBench).await
}
