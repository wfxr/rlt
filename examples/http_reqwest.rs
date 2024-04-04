use anyhow::Result;
use async_trait::async_trait;
use clap::Parser;
use reqwest::{Client, Url};
use rlt::{
    cli::BenchCli,
    IterReport, Status, {BenchSuite, IterInfo},
};
use tokio::time::Instant;

#[derive(Parser, Clone)]
pub struct Opts {
    /// Target URL.
    pub url: Url,

    /// Embed BenchOpts into this Opts.
    #[command(flatten)]
    pub bench_opts: BenchCli,
}

#[derive(Clone)]
struct HttpBench {
    url: Url,
}

#[async_trait]
impl BenchSuite for HttpBench {
    type WorkerState = Client;

    async fn state(&self, _: u32) -> Result<Self::WorkerState> {
        Ok(Client::new())
    }

    async fn bench(&mut self, client: &mut Self::WorkerState, _: &IterInfo) -> Result<IterReport> {
        let t = Instant::now();
        let resp = client.get(self.url.clone()).send().await?;

        let status = match resp.status() {
            s if s.is_success() => Status::success(s.as_u16().into()),
            s if s.is_client_error() => Status::client_error(s.as_u16().into()),
            s if s.is_server_error() => Status::server_error(s.as_u16().into()),
            s => Status::error(s.as_u16().into()),
        };
        let bytes = resp.bytes().await?.len() as u64;
        let duration = t.elapsed();

        Ok(IterReport { duration, status, bytes, items: 1 })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts: Opts = Opts::parse();
    let bench_suite = HttpBench { url: opts.url };
    rlt::cli::run(&opts.bench_opts, bench_suite).await
}
