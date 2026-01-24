# rlt


A **R**ust **L**oad **T**esting framework with real-time tui support.

[![Crates.io](https://img.shields.io/crates/v/rlt.svg)](https://crates.io/crates/rlt)
[![Documentation](https://docs.rs/rlt/badge.svg)](https://docs.rs/rlt/)
[![Dependency status](https://deps.rs/repo/github/wfxr/rlt/status.svg)](https://deps.rs/repo/github/wfxr/rlt)
[![License](https://img.shields.io/crates/l/csview.svg)](https://github.com/wfxr/rlt?tab=MIT-1-ov-file)

![Screenshot](https://raw.githubusercontent.com/wfxr/i/master/rlt-demo.gif)

**rlt** provides a simple way to create load test tools in Rust.
It is designed to be a universal load test framework, which means you can use
rlt for various services, such as Http, gRPC, Thrift, Database, or other customized services.

### Features

- **Flexible**: Customize the work load with your own logic.
- **Easy to use**: Little boilerplate code, just focus on testing.
- **Rich Statistics**: Collect and display rich statistics.
- **High performance**: Optimized for performance and resource usage.
- **Real-time TUI**: Monitor testing progress with a powerful real-time TUI.
- **Baseline Comparison**: Save and compare results to track performance changes.

### Quick Start

Run `cargo add rlt` to add `rlt` as a dependency to your `Cargo.toml`:

```toml
[dependencies]
rlt = "0.3.0"
```

Then create your bench suite by implementing the `BenchSuite` trait.

The `bench_cli!` macro can be used to define your CLI options which
automatically includes the predefined `BenchCli` options.

```rust
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
```

*You can also create a separate struct to hold the cli options for more flexibility. There is an example in [examples/http_hyper.rs](examples/http_hyper.rs).*

Finally, create the main function to run the load test using the `bench_cli_run!` macro.

```rust
#[tokio::main]
async fn main() -> Result<()> {
    bench_cli_run!(HttpBench).await
}
```

More examples can be found in the [examples](examples) directory.

### Baseline Comparison

rlt supports saving benchmark results as baselines and comparing future runs against them:

```bash
# Save current run as baseline
mybench --url http://localhost:8080 -c 10 -d 30s --save-baseline v1.0

# Compare against baseline
mybench --url http://localhost:8080 -c 10 -d 30s --baseline v1.0

# CI mode: fail on regression
mybench --url http://localhost:8080 --baseline main --fail-on-regression
```

The comparison displays performance deltas with color-coded indicators, making it easy to track performance changes across code revisions.

### Credits

The TUI layout in rlt is inspired by [oha](https://github.com/hatoo/oha).

### License

`rlt` is distributed under the terms of both the MIT License and the Apache License 2.0.

See the [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) files for license details.
