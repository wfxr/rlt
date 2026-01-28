//! Human-readable text reporter with colored output.
//!
//! This module provides [`TextReporter`], which formats benchmark results
//! as colorful, structured text suitable for terminal display.
//!
//! # Output Sections
//!
//! The text report includes:
//! - **Summary**: Total time, concurrency, success rate, and throughput metrics
//! - **Latencies**: Statistics (avg, min, med, max, stdev), percentiles, and histogram
//! - **Status Distribution**: Breakdown of response status codes
//! - **Error Distribution**: List of errors with counts (if any)
//! - **Baseline Comparison**: Performance changes vs baseline (if provided)
//!
//! # Colors
//!
//! The reporter uses ANSI colors to highlight important information:
//! - Green: Success indicators, positive values
//! - Yellow: Warnings, section headers
//! - Red: Errors, regressions
//! - Cyan: Labels, informational text

use crossterm::style::{StyledContent, Stylize};
use itertools::Itertools;
use std::{cmp::Reverse, collections::HashMap, io::Write};
use tabled::settings::Padding;
use tabled::settings::PaddingColor;
use tabled::settings::object::{Cell, Columns, FirstColumn, FirstRow, LastColumn, Object, Rows};
use tabled::{
    builder::Builder,
    settings::{Alignment, Color, Margin, Style, themes::Colorization},
};

use crate::baseline::{Comparison, Delta, DeltaStatus, LatencyDeltas, RegressionMetric, Verdict};
use crate::duration::TimeUnit;
use crate::{
    duration::{DurationExt, FormattedDuration},
    histogram::{LatencyHistogram, PERCENTAGES},
    report::BenchReport,
    status::{Status, StatusKind},
    util::{IntoAdjustedByte, TryIntoAdjustedByte, rate},
};

/// A text reporter that outputs human-readable, colored benchmark results.
///
/// This reporter formats results using ANSI terminal colors and structured
/// tables, making it ideal for interactive terminal use.
///
/// # Example
///
/// ```ignore
/// use rlt::reporter::{BenchReporter, TextReporter};
///
/// let reporter = TextReporter;
/// reporter.print(&mut std::io::stdout(), &report, None)?;
/// ```
pub struct TextReporter;

impl super::BenchReporter for TextReporter {
    fn print(&self, w: &mut dyn Write, report: &BenchReport, comparison: Option<&Comparison>) -> anyhow::Result<()> {
        print_summary(w, report)?;

        if report.stats.overall.iters > 0 {
            writeln!(w)?;
            print_latency(w, &report.hist)?;

            writeln!(w)?;
            print_status(w, &report.status_dist)?;
        }

        if !report.error_dist.is_empty() {
            writeln!(w)?;
            print_error(w, report)?;
        }

        // Print baseline comparison if available
        if let Some(cmp) = comparison {
            // Use the same time unit as latency section
            let u = report.hist.median().appropriate_unit();
            writeln!(w)?;
            print_baseline_comparison(w, cmp, u)?;
        }

        Ok(())
    }
}

fn render_success_ratio(success_rate: f64) -> StyledContent<String> {
    let text = format!("{:.2}%", success_rate);
    if success_rate >= 100.0 {
        text.green().bold()
    } else if success_rate >= 99.0 {
        text.yellow().bold()
    } else {
        text.red().bold()
    }
}

fn print_latency_histogram(
    w: &mut dyn Write,
    hist: &LatencyHistogram,
    u: TimeUnit,
    indent: usize,
) -> anyhow::Result<()> {
    let quantiles = hist
        .quantiles()
        .map(|(latency, count)| (format!("{:.2}", FormattedDuration::from(latency, u)), count))
        .collect_vec();
    if quantiles.is_empty() {
        return Ok(());
    }

    let &max_count = quantiles.iter().map(|(_, count)| count).max().unwrap();
    let quantiles = quantiles
        .into_iter()
        .map(|(latency, count)| vec![count.to_string(), latency, "🭵".into(), render_bar(count, max_count)]);
    let mut quantiles = Builder::from_iter(quantiles).build();
    quantiles
        .with(Style::empty())
        .with(Margin::new(indent * 2, 0, 0, 0))
        .with(Alignment::right())
        .with(Padding::new(0, 1, 0, 0))
        .with(Colorization::exact([Color::FG_GREEN], Columns::new(0..=1)))
        .with(Colorization::exact([Color::FG_GREEN], LastColumn))
        .modify(Columns::new(2..=2), Padding::new(0, 0, 0, 0))
        .modify(LastColumn, Alignment::left())
        .modify(FirstColumn, Padding::new(1, 1, 0, 0).fill('[', ']', ' ', ' '))
        // can be removed?
        .modify(FirstColumn, PaddingColor::filled(Color::default()))
        .modify(Columns::new(1..=1), Padding::new(1, 1, 0, 0));
    writeln!(w, "{}", quantiles)?;

    Ok(())
}

fn render_bar(count: u64, max_count: u64) -> String {
    let ratio = count as f64 / max_count as f64;
    let len = 32.0 * ratio;
    let mut bar = "■".repeat(len as usize);
    if len.fract() >= 0.5 {
        bar.push('◧');
    }
    bar
}

#[rustfmt::skip]
fn print_summary(w: &mut dyn Write, report: &BenchReport) -> anyhow::Result<()> {
    let elapsed = report.elapsed.as_secs_f64();
    let overall = &report.stats.overall;

    writeln!(w, "{}", "Summary".h1())?;
    writeln!(w, "  Benchmark took {} with concurrency {} ({} success)",
                        format!("{:.2}s", elapsed).yellow().bold(),
                        format!("{}", report.concurrency).cyan().bold(),
                        render_success_ratio(100.0 * report.success_ratio()))?;
    writeln!(w)?;

    let stats = vec![
        vec!["".into(), "Total".into(), "Rate".into()],
        vec![
            "Iters".into(),
            format!("{}", overall.iters),
            format!("{:.2}/s", rate(overall.iters, elapsed)),
        ],
        vec![
            "Items".into(),
            format!("{}", overall.items),
            format!("{:.2}/s", rate(overall.items, elapsed)),
        ],
        vec![
            "Bytes".into(),
            format!("{:.2}", overall.bytes.adjusted()),
            format!("{:.2}/s", rate(overall.bytes, elapsed).adjusted()?),
        ],
    ];
    let mut stats = Builder::from(stats).build();
    stats
        .with(Style::empty())
        .with(Alignment::right())
        .with(Padding::new(2, 2, 0, 0))
        .with(Colorization::exact([Color::BOLD], Cell::new(0, 1)))
        .with(Colorization::exact([Color::BOLD], Cell::new(0, 2)))
        .with(Colorization::exact([Color::FG_GREEN], Rows::new(1..=4).not(Columns::new(0..=0))))
        .modify(FirstRow, Alignment::center())
    ;

    writeln!(w, "{}", stats)?;
    Ok(())
}

fn print_latency(w: &mut dyn Write, hist: &LatencyHistogram) -> anyhow::Result<()> {
    writeln!(w, "{}", "Latencies".h1())?;
    if hist.is_empty() {
        return Ok(());
    }

    // time unit for the histogram
    let u = hist.median().appropriate_unit();

    writeln!(w, "{}", "  Stats".h2())?;
    print_latency_stats(w, hist, u)?;
    writeln!(w)?;

    writeln!(w, "{}", "  Percentiles".h2())?;
    print_latency_percentiles(w, hist, u)?;
    writeln!(w)?;

    writeln!(w, "{}", "  Histogram".h2())?;
    print_latency_histogram(w, hist, u, 2)?;

    Ok(())
}

fn print_latency_stats(w: &mut dyn Write, hist: &LatencyHistogram, u: TimeUnit) -> anyhow::Result<()> {
    let stats = vec![
        vec!["Avg".into(), "Min".into(), "Med".into(), "Max".into(), "Stdev".into()],
        vec![
            format!("{:.2}", FormattedDuration::from(hist.mean(), u)),
            format!("{:.2}", FormattedDuration::from(hist.min(), u)),
            format!("{:.2}", FormattedDuration::from(hist.median(), u)),
            format!("{:.2}", FormattedDuration::from(hist.max(), u)),
            format!("{:.2}", FormattedDuration::from(hist.stdev(), u)),
        ],
    ];
    let mut stats = Builder::from(stats).build();
    stats
        .with(Style::empty())
        .with(Margin::new(2, 0, 0, 0))
        .with(Padding::new(2, 2, 0, 0))
        .with(Alignment::center())
        .with(Colorization::exact([Color::FG_GREEN], Rows::new(1..=1)))
        .with(Colorization::exact([Color::FG_BLUE], Cell::new(0, 0)))
        .with(Colorization::exact([Color::FG_CYAN], Cell::new(0, 1)))
        .with(Colorization::exact([Color::FG_YELLOW], Cell::new(0, 2)))
        .with(Colorization::exact([Color::FG_RED], Cell::new(0, 3)))
        .with(Colorization::exact([Color::FG_MAGENTA], Cell::new(0, 4)));
    writeln!(w, "{}", stats)?;
    Ok(())
}

fn print_latency_percentiles(w: &mut dyn Write, hist: &LatencyHistogram, u: TimeUnit) -> anyhow::Result<()> {
    let percentiles = hist.percentiles(PERCENTAGES).map(|(p, v)| {
        vec![
            format!("{:.2}%", p),
            format!(" in "),
            format!("{:.2}", FormattedDuration::from(v, u)),
        ]
    });
    let mut percentiles = Builder::from_iter(percentiles).build();
    percentiles
        .with(Style::empty())
        .with(Margin::new(4, 0, 0, 0))
        .with(Alignment::center())
        .with(Padding::zero())
        .with(Colorization::exact([Color::FG_GREEN], FirstColumn))
        .with(Colorization::exact([Color::FG_GREEN], LastColumn))
        .modify(LastColumn, Alignment::right());
    writeln!(w, "{}", percentiles)?;
    Ok(())
}

fn print_status(w: &mut dyn Write, status: &HashMap<Status, u64>) -> anyhow::Result<()> {
    let status_v = status
        .iter()
        .sorted_unstable_by_key(|&(_, cnt)| Reverse(cnt))
        .collect_vec();
    writeln!(w, "{}", "Status Distribution".h1())?;
    if !status_v.is_empty() {
        let max = status_v.iter().map(|(_, iters)| iters).max().unwrap();
        let count_width = max.to_string().len();
        for (&status, count) in status_v {
            let count = format!("{count:>count_width$}").green();
            let status = match status.kind() {
                StatusKind::Success => status.to_string().green(),
                StatusKind::ClientError => status.to_string().yellow(),
                StatusKind::ServerError => status.to_string().red(),
                StatusKind::Error => status.to_string().red(),
            };

            writeln!(w, "  [{count}] {status}")?;
        }
    }
    Ok(())
}

fn print_error(w: &mut dyn Write, report: &BenchReport) -> anyhow::Result<()> {
    let error_v = report
        .error_dist
        .iter()
        .sorted_unstable_by_key(|&(_, cnt)| Reverse(cnt))
        .collect_vec();
    let max = error_v.iter().map(|(_, iters)| iters).max().unwrap();
    let iters_width = max.to_string().len();
    writeln!(w, "{}", "Error Distribution".h1())?;
    for (error, count) in error_v {
        writeln!(w, "{}", format!("  [{count:>iters_width$}] {error}").red())?;
    }
    Ok(())
}

trait ReportStyle {
    fn h1(&self) -> StyledContent<&str>;
    fn h2(&self) -> StyledContent<&str>;
}

impl<T: AsRef<str>> ReportStyle for T {
    fn h1(&self) -> StyledContent<&str> {
        self.as_ref().bold().underlined().yellow()
    }

    fn h2(&self) -> StyledContent<&str> {
        self.as_ref().bold().cyan()
    }
}

fn format_rate(value: f64, metric: RegressionMetric) -> String {
    match metric {
        RegressionMetric::SuccessRatio => format!("{:.2}%", value * 100.0),
        RegressionMetric::BytesRate => match value.adjusted() {
            Ok(adjusted) => format!("{:.2}/s", adjusted),
            Err(_) => format!("{:.2}/s", value),
        },
        _ => format!("{:.2}/s", value),
    }
}

fn format_latency(secs: f64, u: TimeUnit) -> String {
    use std::time::Duration;
    let d = Duration::from_secs_f64(secs);
    format!("{:.2}", FormattedDuration::from(d, u))
}

fn format_delta_change(delta: &Delta) -> String {
    match delta.status {
        DeltaStatus::Unchanged => "no change".dim().to_string(),
        DeltaStatus::Improved => {
            let pct = delta.delta.unwrap_or(0.0);
            format!("{:+.2}% better", pct).green().to_string()
        }
        DeltaStatus::Regressed => {
            let pct = delta.delta.unwrap_or(0.0);
            format!("{:+.2}% worse ", pct).red().to_string()
        }
    }
}

fn format_metric_name(
    metric: RegressionMetric,
    regression_metrics: &[RegressionMetric],
    skipped_metrics: &[RegressionMetric],
) -> String {
    // Only mark with star if selected AND not skipped (i.e., actually used for verdict)
    let is_used = regression_metrics.contains(&metric) && !skipped_metrics.contains(&metric);
    let prefix = if is_used { "* " } else { "  " };
    format!("{}{}", prefix, metric.display_name())
}

fn print_baseline_comparison(w: &mut dyn Write, cmp: &Comparison, u: TimeUnit) -> anyhow::Result<()> {
    writeln!(w, "{}", "Baseline Comparison".h1())?;

    // Summary line
    let verdict_str = cmp.verdict.to_string();
    let verdict_str = match cmp.verdict {
        Verdict::Improved => verdict_str.green().bold(),
        Verdict::Regressed => verdict_str.red().bold(),
        Verdict::Unchanged => verdict_str.yellow(),
        Verdict::Mixed => verdict_str.yellow().bold(),
    };
    writeln!(
        w,
        "  Compared with baseline {} using {:.1}% noise threshold ({})",
        cmp.baseline_name.clone().green().bold(),
        cmp.noise_threshold,
        verdict_str
    )?;
    writeln!(w)?;

    // Throughput comparison
    writeln!(w, "{}", "  Throughput".h2())?;
    print_throughput_comparison(w, cmp)?;
    writeln!(w)?;

    // Latency comparison (if available)
    if let Some(ref deltas) = cmp.latency {
        writeln!(w, "{}", "  Latency".h2())?;
        print_latency_comparison(w, deltas, u, &cmp.regression_metrics, &cmp.skipped_metrics)?;
        writeln!(w)?;
    }

    // Reliability comparison (success ratio)
    writeln!(w, "{}", "  Reliability".h2())?;
    print_reliability_comparison(w, cmp)?;
    writeln!(w)?;

    // Skipped metrics warning (if any)
    if !cmp.skipped_metrics.is_empty() {
        let skipped = cmp.skipped_metrics.iter().map(|m| m.display_name()).join(", ");
        let msg = format!(
            "Note: Some metrics for verdict calculation were unavailable: {}",
            skipped
        );
        writeln!(w, "  {}", msg.dim().italic())?;
        writeln!(w)?;
    }

    // Footnote
    writeln!(w, "  {}", "* Metrics for verdict calculation".italic())?;

    Ok(())
}

fn print_throughput_comparison(w: &mut dyn Write, cmp: &Comparison) -> anyhow::Result<()> {
    let throughput = &cmp.throughput;
    let mut rows = vec![(RegressionMetric::ItersRate, &throughput.iters_rate)];
    if let Some(ref items_rate) = throughput.items_rate {
        rows.push((RegressionMetric::ItemsRate, items_rate));
    }
    if let Some(ref bytes_rate) = throughput.bytes_rate {
        rows.push((RegressionMetric::BytesRate, bytes_rate));
    }
    print_comparison_table(w, &rows, format_rate, &cmp.regression_metrics, &cmp.skipped_metrics)
}

fn print_latency_comparison(
    w: &mut dyn Write,
    deltas: &LatencyDeltas,
    u: TimeUnit,
    regression_metrics: &[RegressionMetric],
    skipped_metrics: &[RegressionMetric],
) -> anyhow::Result<()> {
    let mut rows = vec![
        (RegressionMetric::LatencyMean, &deltas.mean),
        (RegressionMetric::LatencyMedian, &deltas.median),
    ];
    if let Some(ref p90) = deltas.p90 {
        rows.push((RegressionMetric::LatencyP90, p90));
    }
    if let Some(ref p99) = deltas.p99 {
        rows.push((RegressionMetric::LatencyP99, p99));
    }
    rows.push((RegressionMetric::LatencyMax, &deltas.max));
    print_comparison_table(
        w,
        &rows,
        |v, _| format_latency(v, u),
        regression_metrics,
        skipped_metrics,
    )
}

fn print_reliability_comparison(w: &mut dyn Write, cmp: &Comparison) -> anyhow::Result<()> {
    let rows = [(RegressionMetric::SuccessRatio, &cmp.success_ratio)];
    print_comparison_table(w, &rows, format_rate, &cmp.regression_metrics, &cmp.skipped_metrics)
}

fn print_comparison_table<F>(
    w: &mut dyn Write,
    rows: &[(RegressionMetric, &Delta)],
    format_value: F,
    regression_metrics: &[RegressionMetric],
    skipped_metrics: &[RegressionMetric],
) -> anyhow::Result<()>
where
    F: Fn(f64, RegressionMetric) -> String,
{
    let mut builder = Builder::default();
    builder.push_record(["Metric", "Current", "Baseline", "Change"]);
    for (metric, delta) in rows {
        builder.push_record([
            format_metric_name(*metric, regression_metrics, skipped_metrics),
            format_value(delta.current, *metric),
            format_value(delta.baseline, *metric),
            format_delta_change(delta),
        ]);
    }

    let mut table = builder.build();
    table
        .with(Style::empty())
        .with(Margin::new(2, 0, 0, 0))
        .with(Alignment::right())
        .with(Padding::new(2, 2, 0, 0))
        .with(Colorization::exact([Color::BOLD], FirstRow))
        .modify(FirstColumn, Alignment::left())
        .modify(FirstRow, Alignment::center());

    writeln!(w, "{}", table)?;
    Ok(())
}
