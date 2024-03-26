mod silent;
mod tui;

use async_trait::async_trait;

pub use silent::SilentCollector;
pub use tui::TuiCollector;

use crate::report::BenchReport;

#[async_trait]
pub trait ReportCollector: Send + Sync {
    async fn run(&mut self) -> anyhow::Result<BenchReport>;
}
