# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**rlt** is a Rust Load Testing framework with real-time TUI support. It provides a universal load testing library for HTTP, gRPC, Thrift, database, and custom services.

## Build Commands

```bash
cargo build                      # Build debug binary
cargo build --release            # Build release binary
cargo test --all-features        # Run all tests
cargo test --all-features --doc  # Run doc tests
cargo fmt --check                # Check formatting
cargo clippy                     # Run linter
```

## Architecture

### Core Traits

**`BenchSuite`** (src/runner.rs) - For stateful benchmarks with per-worker state:
- `setup(worker_id)` - Initialize and return worker state (e.g., HTTP client, DB connection)
- `bench(state, info)` - Run single iteration, return `IterReport`
- `teardown(state, info)` - Optional cleanup per worker

**`StatelessBenchSuite`** (src/runner.rs) - Simpler trait for stateless benchmarks:
- `bench(info)` - Run single iteration, return `IterReport`
- Automatically implements `BenchSuite` with `WorkerState = ()`

### Key Types

- `IterReport` - Result of single iteration: duration, status, bytes, items
- `BenchReport` - Aggregated final results with histograms and statistics
- `Status` / `StatusKind` - HTTP-compatible status tracking
- `IterInfo` - Context passed to bench: worker_id, worker_seq, runner_seq

### Module Structure

- `cli.rs` - CLI parsing with `BenchCli` struct and `bench_cli!`/`bench_cli_run!` macros
- `runner.rs` - `Runner` orchestrates workers, handles warmup, rate limiting, cancellation
- `collector/` - Real-time result collection: `TuiCollector`, `SilentCollector`
- `reporter/` - Final report output: `TextReporter`, `JsonReporter`
- `stats/` - Statistics tracking with rolling windows and counters

### Execution Flow

1. CLI parsed → `BenchCli` with concurrency, iterations, duration, warmup, rate
2. Clock created in paused state
3. Workers spawned, each calls `setup()`
4. Warmup iterations run (results discarded)
5. Barrier sync → clock resumed → main benchmark starts
6. Results sent via mpsc channel to collector
7. Duration/iteration limit reached → workers call `teardown()`
8. Report generated and output

## Cargo Features

- `default = ["tracing", "rate_limit", "http"]`
- `tracing` - Logging via tui-logger
- `rate_limit` - Rate limiting via governor crate
- `http` - HTTP status code conversion

## Code Style

- Max line width: 120 characters (see rustfmt.toml)
- Uses `rlt::Result` for framework errors, `rlt::BenchResult` (backed by `anyhow::Error`) for user benchmarks
- Async-first with tokio runtime
- Uses `async-trait` for async trait methods

## Examples

Run examples with:
```bash
cargo run --example simple_stateless -- -c 4 -d 10s
cargo run --example http_reqwest -- --url http://example.com -c 10
```
