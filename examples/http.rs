use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use clap::Parser;
use http_body_util::{BodyExt, Full};
use hyper::Uri;
use hyper_tls::HttpsConnector;
use hyper_util::{
    client::legacy::{connect::HttpConnector, Client},
    rt::TokioExecutor,
};
use rlt::{
    cli::BenchCli,
    IterReport, Status, {BenchSuite, IterInfo},
};
use tokio::time::Instant;

#[derive(Parser, Clone)]
pub struct Opts {
    /// Target URL.
    pub url: Uri,

    /// Embed BenchOpts into this Opts.
    #[command(flatten)]
    pub bench_opts: BenchCli,
}

#[derive(Clone)]
struct HttpBench {
    url: Uri,
}

#[async_trait]
impl BenchSuite for HttpBench {
    type WorkerState = Client<HttpsConnector<HttpConnector>, Full<Bytes>>;

    async fn state(&self, _: u32) -> Result<Self::WorkerState> {
        let https = HttpsConnector::new();
        let client = Client::builder(TokioExecutor::new()).build(https);
        Ok(client)
    }

    async fn bench(&mut self, client: &mut Self::WorkerState, _: &IterInfo) -> Result<IterReport> {
        let t = Instant::now();
        let mut resp = client.get(self.url.clone()).await?;

        let status = match resp.status() {
            s if s.is_success() => Status::success(s.as_u16().into()),
            s if s.is_client_error() => Status::client_error(s.as_u16().into()),
            s if s.is_server_error() => Status::server_error(s.as_u16().into()),
            s => Status::error(s.as_u16().into()),
        };

        let mut bytes = 0;
        while let Some(next) = resp.frame().await {
            bytes += next?.data_ref().map(Bytes::len).unwrap_or_default() as u64;
        }
        let duration = t.elapsed();

        Ok(IterReport { duration, status, bytes, items: 1 })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts: Opts = Opts::parse();

    let bench_suite = HttpBench { url: opts.url };

    rlt::cli::run(&opts.bench_opts, bench_suite).await?;

    Ok(())
}
