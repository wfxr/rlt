# RFC 0001: Baseline Support for Benchmark Reports

- **Start Date:** 2026-01-23

## Summary

Add baseline support to rlt, enabling users to save benchmark results and compare future runs against them. The comparison will display performance deltas with color-coded indicators (green for improvements, red for regressions), similar to [criterion-rs](https://github.com/bheisler/criterion.rs) and [hyperfine](https://github.com/sharkdp/hyperfine).

## Motivation

Load testing often involves iterative optimization. Users need to:

1. **Track performance changes** - Understand if code changes improved or degraded performance
2. **Prevent regressions** - Detect performance regressions early in CI/CD pipelines
3. **Quantify improvements** - Measure the impact of optimizations

Currently, rlt produces standalone reports with no historical context. Users must manually compare JSON outputs or rely on external tools. By adding native baseline support, rlt provides a seamless experience for performance tracking.

### Prior Art

| Tool | Baseline Storage | Comparison Display | Statistical Analysis |
|------|------------------|-------------------|---------------------|
| [criterion-rs](https://bheisler.github.io/criterion.rs/book/) | Directory with JSON files | Confidence intervals + message | p-value, noise threshold |
| [hyperfine](https://github.com/sharkdp/hyperfine) | JSON export | Speed-up factor | Mean, stddev, outlier detection |
| [k6](https://grafana.com/docs/grafana-cloud/testing/k6/) | Cloud storage | Threshold-based pass/fail | Percentile thresholds |
| [vegeta](https://github.com/tsenart/vegeta) | Binary/JSON | HdrHistogram plots | Percentile distribution |

## Detailed Design

### 1. CLI Interface

New CLI options for baseline management:

| Option | Description |
|--------|-------------|
| `--save-baseline <NAME>` | Save benchmark results as a named baseline |
| `--baseline <NAME>` | Compare against a named baseline |
| `--baseline-file <PATH>` | Compare against a JSON file (mutually exclusive with `--baseline`) |
| `--baseline-dir <PATH>` | Directory for storing baselines |
| `--noise-threshold <PERCENT>` | Noise threshold for comparison (default: 1.0) |
| `--fail-on-regression` | Exit with code 1 if regression detected |
| `--regression-metrics <METRICS>` | Comma-separated list of metrics for verdict calculation |

#### Baseline Name Validation

Baseline names must match the pattern `[a-zA-Z0-9_.-]+`. Invalid names are rejected at CLI parse time.

#### Baseline Directory Resolution

Priority order:
1. `--baseline-dir` CLI flag
2. `RLT_BASELINE_DIR` environment variable
3. `${CARGO_TARGET_DIR}/rlt/baselines`
4. `target/rlt/baselines` (default)

#### Example Usage

```bash
# Save current run as baseline "v1.0"
mybench --url http://localhost:8080 -c 10 -d 30s --save-baseline v1.0

# Compare against baseline "v1.0"
mybench --url http://localhost:8080 -c 10 -d 30s --baseline v1.0

# Compare against main and save as feature-branch
mybench --url http://localhost:8080 -c 10 -d 30s --baseline main --save-baseline feature-branch

# CI mode: fail on regression
mybench --url http://localhost:8080 --baseline main --fail-on-regression

# Custom regression metrics
mybench --url http://localhost:8080 --baseline main \
    --regression-metrics iters-rate,latency-mean,latency-p99
```

#### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success (no comparison, or verdict is `improved`/`unchanged`) |
| 1 | Regression detected (when `--fail-on-regression` is set); or error |

### 2. Baseline Storage Format

Baselines are stored as JSON files with schema versioning:

```json
{
  "schema_version": 1,
  "metadata": {
    "name": "v1.0",
    "created_at": "2026-01-23T10:30:00Z",
    "rlt_version": "0.4.0",
    "bench_config": {
      "concurrency": 10,
      "duration_secs": 30.0,
      "iterations": null,
      "warmup": 100,
      "rate_limit": null,
      "actual_duration_secs": 30.05
    }
  },
  "summary": { ... },
  "latency": { ... },
  "status": { ... },
  "errors": { ... }
}
```

#### Design Decisions

- **No command recording**: The baseline format does not record the original command line to avoid accidentally persisting sensitive information (tokens, passwords, API keys).
- **Overwrite by default**: `--save-baseline` overwrites existing baselines without requiring `--force`, matching criterion-rs behavior.
- **Atomic writes**: Baseline files are written atomically using write-to-temp-then-rename to prevent corruption.

### 3. Comparability Validation

Before running the benchmark, the tool validates that the baseline is comparable:

| Parameter | Mismatch Behavior |
|-----------|-------------------|
| `concurrency` | **Error** - results not comparable |
| `rate_limit` | **Error** - different load profiles |

Validation happens before benchmark execution to fail fast.

### 4. Comparison Metrics

| Category | Metric | Better When | Default |
|----------|--------|-------------|---------|
| Throughput | iters-rate | Higher | Yes |
| Throughput | items-rate | Higher | No |
| Throughput | bytes-rate | Higher | No |
| Latency | latency-mean | Lower | Yes |
| Latency | latency-median | Lower | No |
| Latency | latency-p90 | Lower | Yes |
| Latency | latency-p99 | Lower | Yes |
| Latency | latency-max | Lower | No |
| Reliability | success-ratio | Higher | Yes |

#### Verdict Calculation

Based on regression metrics only (default or user-specified via `--regression-metrics`):

| Condition | Verdict |
|-----------|---------|
| At least one improved, none regressed | `improved` |
| At least one regressed, none improved | `regressed` |
| All unchanged | `unchanged` |
| Some improved, some regressed | `mixed` |

`mixed` is treated as regression for `--fail-on-regression`.

#### Delta Calculation

- **All metrics**: Percentage change `(current - baseline) / baseline * 100`
- **Noise threshold**: Changes within threshold are reported as "unchanged"

### 5. Report Output

#### Text Format

After the main report sections, a "Baseline Comparison" section is displayed:

```
Baseline Comparison
  Compared with baseline v1.0 using 1.0% noise threshold (improved)

  Throughput
        Metric      Current      Baseline      Change
    * Iters/s      1234.56/s    1100.00/s     +12.23% better

  Latency
        Metric      Current      Baseline      Change
    *    Avg        1.82ms       1.99ms        -8.54% better
         Med        1.71ms       1.82ms        -6.04% better
    *    p90        2.98ms       3.12ms        -4.49% better
    *    p99        5.89ms       6.23ms        -5.46% better
         Max       82.34ms      89.23ms        -7.72% better

  Reliability
        Metric      Current      Baseline      Change
    * Success       99.50%       99.00%        +0.51% better

  * Metrics selected for verdict calculation
```

The `*` prefix marks metrics that are included in `--regression-metrics` and used for verdict calculation.

| Change | Display | Color |
|--------|---------|-------|
| Improved | `+12.34% better` | Green |
| Regressed | `-12.34% worse ` | Red |
| Within noise | `no change` | Dim |

#### JSON Format

When a baseline is provided, the JSON output includes a `comparison` field with throughput, latency, and success_ratio deltas.

## Alternatives Considered

### External Tool Approach
A separate CLI tool `rlt-cmp` for comparison. Rejected for worse UX and harder CI/CD integration.

### Statistical Confidence Intervals
Use bootstrap/t-test for rigorous analysis. Deferred - start with simple percentage comparison + noise threshold.

### Database Storage
Store baselines in SQLite. Rejected - JSON files are simpler and portable.

### Feature Flag
Gate baseline behind a cargo feature. Rejected - conditional compilation complexity outweighs benefits.

## Open Questions

1. **Multiple baseline comparison** - e.g., `--baseline v1.0 --baseline v2.0`. Defer to v2.
2. **List baselines** - `--list-baselines` subcommand for discovering available baselines.
3. **Baseline cleanup** - `--baseline-max-age 30d` to auto-delete old baselines.

## References

- [Criterion.rs User Guide](https://bheisler.github.io/criterion.rs/book/)
- [Hyperfine - Command-line Benchmarking](https://github.com/sharkdp/hyperfine)
- [k6 Test Comparison](https://grafana.com/docs/grafana-cloud/testing/k6/analyze-results/test-comparison/)
