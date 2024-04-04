//! A universal load testing library for Rust, with real-time tui support.
//!
//! This crate provides a simple and flexible way to create a load test tool in rust.
//! It is designed to be a universal load test framework, which means you can use
//! rlt to create load test tools for different kinds of services, such as http, grpc,
//! database, or more complex and customized services.
//!
//! ## Features
//!
//! - **Flexible**: Customize the load test scenario with your own logic.
//! - **Easy to use**: Little boilerplate code, just focus on testing logic.
//! - **Rich Statistics**: Collect and display rich statistics during the load test.
//! - **High performance**: Carefully optimized for performance and resource usage.
//! - **Real-time TUI**: Monitor the progress of the load test with a powerful real-time TUI.
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
//!     rlt::cli::run(&BenchCli::parse(), SimpleBench).await
//! }
//! ```
//!
//! Stateful bench is also supported, see the [examples/http](https://github.com/wfxr/rlt/blob/main/examples/http.rs).
#![deny(missing_docs)]

mod duration;
mod histogram;
mod report;
mod runner;
mod stats;
mod status;
mod util;

pub mod cli;
pub mod collector;
pub mod reporter;

pub use crate::{
    report::BenchReport,
    report::IterReport,
    runner::IterInfo,
    runner::{BenchSuite, StatelessBenchSuite},
    status::Status,
};
