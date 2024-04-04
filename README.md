# rlt


A flexible **R**ust **L**oad **T**esting framework, with real-time tui support.

[![Crates.io](https://img.shields.io/crates/v/rlt.svg)](https://crates.io/crates/rlt)
[![Documentation](https://docs.rs/rlt/badge.svg)](https://docs.rs/rlt/)
[![Dependency status](https://deps.rs/repo/github/wfxr/rlt/status.svg)](https://deps.rs/repo/github/wfxr/rlt)
[![License](https://img.shields.io/crates/l/csview.svg)](https://github.com/wfxr/rlt?tab=MIT-1-ov-file)

![Screenshot](https://raw.githubusercontent.com/wfxr/i/master/rlt-demo.gif)

**rlt** provides a simple and flexible way to create a load test tool in rust.
It is designed to be a universal load test framework, which means you can use
rlt to create load test tools for different kinds of services, such as http, grpc,
database, or more complex and customized services.

### Features

- **Flexible**: Customize the load test scenario with your own logic.
- **Easy to use**: Little boilerplate code, just focus on testing logic.
- **Rich Statistics**: Collect and display rich statistics during the load test.
- **High performance**: Carefully optimized for performance and resource usage.
- **Real-time TUI**: Monitor the progress of the load test with a powerful real-time TUI.

### License

`rlt` is distributed under the terms of both the MIT License and the Apache License 2.0.

See the [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) files for license details.
