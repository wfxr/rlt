use anyhow::Result;
use async_trait::async_trait;
use clap::Parser;
use rlt::{cli::BenchCli, BenchSuite, IterInfo, IterReport, Status};
use tokio::time::Instant;
use tokio_postgres::{Client, NoTls};

#[derive(Parser, Clone)]
pub struct DBBench {
    /// Host of the PostgreSQL server.
    #[clap(long, default_value = "localhost")]
    pub host: String,

    /// Port of the PostgreSQL server.
    #[clap(long, default_value_t = 5432)]
    pub port: u16,

    /// Username for authentication.
    #[clap(long, default_value = "postgres")]
    pub user: String,

    /// Password for authentication.
    #[clap(long)]
    pub password: Option<String>,

    /// Number of rows to insert in each batch.
    #[clap(long, short = 'b')]
    pub batch_size: u32,

    /// Name of the table to insert into.
    #[clap(long, default_value = "t")]
    pub table: String,

    /// Embed BenchCli into this Opts.
    #[command(flatten)]
    pub bench_opts: BenchCli,
}

#[async_trait]
impl BenchSuite for DBBench {
    type WorkerState = Client;

    async fn state(&self, _: u32) -> Result<Self::WorkerState> {
        let (client, conn) = tokio_postgres::connect(
            &format!(
                "host={} port={} user={} password='{}'",
                self.host,
                self.port,
                self.user,
                self.password.as_deref().unwrap_or_default()
            ),
            NoTls,
        )
        .await?;

        tokio::spawn(async move {
            if let Err(e) = conn.await {
                eprintln!("connection error: {}", e);
            }
        });

        Ok(client)
    }

    async fn setup(&mut self, client: &mut Self::WorkerState, _: u32) -> Result<()> {
        client.execute("BEGIN", &[]).await?;
        client
            .execute("CREATE TABLE t(id SERIAL PRIMARY KEY, name TEXT)", &[])
            .await?;
        Ok(())
    }

    async fn bench(&mut self, client: &mut Self::WorkerState, _: &IterInfo) -> Result<IterReport> {
        let t = Instant::now();
        client
            .query(
                "INSERT INTO t(name) SELECT MD5(i::TEXT) FROM generate_series(1, $1) i",
                &[&(self.batch_size as i32)],
            )
            .await?;
        let duration = t.elapsed();

        Ok(IterReport {
            duration,
            status: Status::success(0),
            bytes: 0,
            items: self.batch_size as u64,
        })
    }

    async fn teardown(self, client: Self::WorkerState, _: IterInfo) -> Result<()> {
        client.execute("ROLLBACK", &[]).await?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let bs: DBBench = DBBench::parse();
    rlt::cli::run(bs.bench_opts, bs).await
}
