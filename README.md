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

### Quick Start

Run `cargo add rlt` to add `rlt` as a dependency to your `Cargo.toml`:

```toml
[dependencies]
rlt = "0.3.0"
```

Then create your bench suite by implementing the `BenchSuite` trait.
`flatten` attribute can be used to embed the predefined `BenchCli` into your own.

```rust
#[derive(Parser, Clone)]
pub struct HttpBench {
    /// Target URL.
    pub url: Url,

    /// Embed BenchCli into this Opts.
    #[command(flatten)]
    pub bench_opts: BenchCli,
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
        let status = resp.status().into();
        let bytes = resp.bytes().await?.len() as u64;
        let duration = t.elapsed();
        Ok(IterReport { duration, status, bytes, items: 1 })
    }
}
```

*You can also create a separate struct to hold the cli options for more flexibility. There is an example in [examples/http_hyper.rs](examples/http_hyper.rs).*

Finally, create the main function to run the load test:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let bs = HttpBench::parse();
    rlt::cli::run(bs.bench_opts.clone(), bs).await
}
```

More examples can be found in the [examples](examples) directory.

### Credits

The TUI layout in rlt is inspired by [oha](https://github.com/hatoo/oha).

### License

`rlt` is distributed under the terms of both the MIT License and the Apache License 2.0.

See the [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) files for license details.
