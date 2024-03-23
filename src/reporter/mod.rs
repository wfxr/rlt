mod json;
mod text;

pub use json::JsonReporter;
pub use text::TextReporter;

use crate::report::BenchReport;

pub trait BenchReporter {
    fn print(&self, w: &mut dyn std::io::Write, report: &BenchReport) -> anyhow::Result<()>;
}
