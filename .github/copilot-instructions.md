# Copilot Instructions for rlt

## Project Overview

rlt is a **R**ust **L**oad **T**esting framework with real-time TUI (Terminal User Interface) support. It provides a flexible, high-performance way to create load testing tools for various services including HTTP, gRPC, Thrift, databases, and other custom services.

### Key Features
- **Flexible**: Customizable workload with user-defined logic
- **Easy to use**: Minimal boilerplate, focus on testing logic
- **Rich Statistics**: Comprehensive statistics collection and display
- **High performance**: Optimized for performance and resource usage
- **Real-time TUI**: Live monitoring with a powerful real-time terminal interface

## Architecture and Code Structure

### Core Components
- `src/lib.rs` - Main library entry point
- `src/cli.rs` - Command-line interface implementation
- `src/runner.rs` - Core load testing runner logic
- `src/collector/` - Statistics collection and TUI components
- `src/reporter/` - Result reporting functionality
- `src/stats/` - Statistics data structures and utilities
- `examples/` - Example implementations for various services

### Key Traits and Types
- `BenchSuite` - Main trait for implementing load test scenarios
- `BenchCli` - Command-line options that can be embedded using `#[command(flatten)]`
- `IterReport` - Structure for reporting individual iteration results
- `WorkerState` - Associated type for maintaining per-worker state

## Development Guidelines

### Code Style and Standards
- **Edition**: Rust 2021 edition
- **Formatting**: Use `cargo fmt` with the project's `rustfmt.toml` configuration
  - Max width: 120 characters
  - Reorder imports enabled
  - Struct literal width: 60 characters
- **Linting**: Code must pass `cargo clippy` checks on stable and beta toolchains
- **MSRV**: Minimum Supported Rust Version is 1.56.1

### Testing and Quality
- All code must compile successfully with `cargo check`
- Run `cargo test --locked --all-features --all-targets` for comprehensive testing
- Documentation tests are included: `cargo test --locked --all-features --doc`
- Use `cargo hack --feature-powerset check` to verify feature combinations

### Dependencies and Features
- **Default features**: `tracing`, `rate_limit`, `http`
- **Optional features**:
  - `tracing`: Logging and tracing support (`log`, `tracing`, `tui-logger`)
  - `rate_limit`: Rate limiting functionality (`governor`)
  - `http`: HTTP support (`http` crate)

### Performance Considerations
- Focus on zero-cost abstractions and efficient memory usage
- Use `async/await` patterns with `tokio` runtime
- Leverage `hdrhistogram` for accurate latency measurements
- Consider using `parking_lot` for synchronization primitives

## Common Patterns

### Implementing a BenchSuite
```rust
#[derive(Parser, Clone)]
pub struct MyBench {
    // Service-specific configuration
    pub target: String,
    
    // Embed standard CLI options
    #[command(flatten)]
    pub bench_opts: BenchCli,
}

#[async_trait]
impl BenchSuite for MyBench {
    type WorkerState = MyClient;

    async fn state(&self, worker_id: u32) -> Result<Self::WorkerState> {
        // Initialize per-worker state
        Ok(MyClient::new(&self.target))
    }

    async fn bench(&mut self, client: &mut Self::WorkerState, info: &IterInfo) -> Result<IterReport> {
        let start = Instant::now();
        // Perform the actual work
        let result = client.do_work().await?;
        let duration = start.elapsed();
        
        Ok(IterReport {
            duration,
            status: result.status_code(),
            bytes: result.response_size(),
            items: 1,
        })
    }
}
```

### Main Function Pattern
```rust
#[tokio::main]
async fn main() -> Result<()> {
    let bench_suite = MyBench::parse();
    rlt::cli::run(bench_suite.bench_opts, bench_suite).await
}
```

## File and Module Guidelines

### Adding New Examples
- Place in `examples/` directory
- Follow the naming pattern `service_client.rs` (e.g., `http_reqwest.rs`)
- Include comprehensive documentation and error handling
- Add any new dependencies to `[dev-dependencies]` in `Cargo.toml`

### TUI Components
- TUI-related code belongs in `src/collector/tui.rs`
- Use `ratatui` for terminal UI components
- Follow the existing layout inspired by the `oha` project
- Handle terminal events and rendering efficiently

### Statistics and Reporting
- Statistics collection in `src/collector/`
- Use `hdrhistogram` for latency measurements
- Support both real-time TUI and final report output
- Follow the established patterns for data aggregation

## Error Handling
- Use `anyhow` for error handling in examples and CLI
- Implement appropriate error types for library code
- Provide meaningful error messages for user-facing failures
- Handle network errors, timeouts, and service-specific failures gracefully

## Documentation Standards
- All public APIs must have comprehensive documentation
- Include examples in doc comments where appropriate
- Use `#[cfg(doc)]` attributes for documentation-only code
- Generate docs with `cargo doc --no-deps --all-features`

## CI/CD Integration
- All changes must pass the complete CI pipeline
- Workflows include: fmt, clippy, doc generation, feature testing, and MSRV checks
- Cross-platform compatibility (Linux, macOS, Windows)
- Code coverage reporting via codecov

## Contribution Guidelines
- Keep changes focused and minimal
- Maintain backward compatibility for public APIs
- Add tests for new functionality
- Update documentation for user-visible changes
- Follow the established code organization patterns

## Common Pitfalls to Avoid
- Don't block the async runtime with synchronous operations
- Avoid unnecessary allocations in hot paths
- Be careful with shared state and synchronization
- Don't forget to handle cancellation and cleanup properly
- Consider rate limiting and backpressure mechanisms