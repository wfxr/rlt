# rlt


A flexible **R**ust **L**oad **T**esting framework with real-time tui support.

[![Crates.io](https://img.shields.io/crates/v/rlt.svg)](https://crates.io/crates/rlt)
[![Documentation](https://docs.rs/rlt/badge.svg)](https://docs.rs/rlt/)
[![Dependency status](https://deps.rs/repo/github/wfxr/rlt/status.svg)](https://deps.rs/repo/github/wfxr/rlt)
[![License](https://img.shields.io/crates/l/csview.svg)](https://github.com/wfxr/rlt?tab=MIT-1-ov-file)

![Screenshot](https://raw.githubusercontent.com/wfxr/i/master/rlt-demo.gif)

**rlt** provides a simple but flexible way to create a load test tool in Rust.
It is designed to be a universal load test framework, which means you can use
rlt to create load test tools for different kinds of services, such as http, grpc,
database, or more complex and customized services.

### Features

- **Flexible**: Customize the load test scenario with your own logic.
- **Easy to use**: Little boilerplate code, just focus on testing logic.
- **Rich Statistics**: Collect and display rich statistics during the load test.
- **High performance**: Carefully optimized for performance and resource usage.
- **Real-time TUI**: Monitor the progress of the load test with a powerful real-time TUI.

### Quick Start

To use `rlt`, firstly add it as a dependency by running `cargo add rlt`.
Then create your bench suite by implementing the `BenchSuite` trait.

```rust
#[derive(Clone)]
struct HttpBench {
    url: Url,
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

Then, create a Opts struct to parse command line arguments.
Note that you can use `flatten` attribute provided by `clap` to embed the predefined `BenchCli` options into your own.

```rust
#[derive(parser, clone)]
pub struct opts {
    /// target url.
    pub url: url,

    /// embed benchopts into this opts.
    #[command(flatten)]
    pub bench_opts: benchcli,
}
```

Finally, create the main function to run the load test.

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let opts: Opts = Opts::parse();
    let bench_suite = HttpBench { url: opts.url };
    rlt::cli::run(&opts.bench_opts, bench_suite).await
}
```

See the [examples](examples) directory for more examples.

### Credits

The TUI layout in rlt is inspired by [oha](https://github.com/hatoo/oha).

### License

`rlt` is distributed under the terms of both the MIT License and the Apache License 2.0.

See the [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) files for license details.
