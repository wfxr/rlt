//! A universal load testing library for Rust, with real-time tui support.
//!
//! This crate provides a simple way to create load test tools in Rust. It is designed
//! to be a universal load test framework, which means you can use rlt for various
//! services, such as Http, gRPC, Thrift, Database, or other customized services.
//!
//! ## Features
//!
//! - **Flexible**: Customize the work load with your own logic.
//! - **Easy to use**: Little boilerplate code, just focus on testing.
//! - **Rich Statistics**: Collect and display rich statistics.
//! - **High performance**: Optimized for performance and resource usage.
//! - **Real-time TUI**: Monitor testing progress with a powerful real-time TUI.
//!
//! ## Example
//!
//! A simple example of a stateless bench suite:
//!
//! ```no_run
//! use anyhow::Result;
//! use async_trait::async_trait;
//! use clap::Parser;
//! use rlt::{cli::BenchCli, IterInfo, IterReport, StatelessBenchSuite, Status};
//! use tokio::time::Instant;
//!
//! #[derive(Clone)]
//! struct SimpleBench;
//!
//! #[async_trait]
//! impl StatelessBenchSuite for SimpleBench {
//!     async fn bench(&mut self, _: &IterInfo) -> Result<IterReport> {
//!         let t = Instant::now();
//!         // do the work here
//!         let duration = t.elapsed();
//!
//!         let report = IterReport {
//!             duration,
//!             status: Status::success(0),
//!             bytes: 42, // bytes processed in current iteration
//!             items: 5,  // items processed in current iteration
//!         };
//!         Ok(report)
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     rlt::cli::run(BenchCli::parse(), SimpleBench).await
//! }
//! ```
//!
//! Stateful bench is also supported, see the [examples/http_reqwest](https://github.com/wfxr/rlt/blob/main/examples/http_reqwest.rs).
#![deny(missing_docs)]

pub mod clock;
mod duration;
mod histogram;
mod phase;
mod report;
mod runner;
mod status;
mod util;

pub(crate) mod stats;

pub mod baseline;
pub mod cli;
pub mod collector;
pub mod reporter;

pub use crate::{
    phase::{BenchPhase, PauseControl, RunState},
    report::BenchReport,
    report::IterReport,
    runner::BenchOpts,
    runner::BenchOptsBuilder,
    runner::IterInfo,
    runner::{BenchSuite, StatelessBenchSuite},
    status::{Status, StatusKind},
};

#[cfg(feature = "tracing")]
pub use tui_logger::TuiTracingSubscriberLayer;
