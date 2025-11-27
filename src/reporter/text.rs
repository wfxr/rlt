use crossterm::style::{StyledContent, Stylize};
use itertools::Itertools;
use std::{cmp::Reverse, collections::HashMap, io::Write};
use tabled::settings::object::{Cell, Columns, FirstColumn, FirstRow, LastColumn, Object, Rows};
use tabled::settings::Padding;
use tabled::settings::PaddingColor;
use tabled::{
    builder::Builder,
    settings::{themes::Colorization, Alignment, Color, Margin, Style},
};

use crate::duration::TimeUnit;
use crate::{
    duration::{DurationExt, FormattedDuration},
    histogram::{LatencyHistogram, PERCENTAGES},
    report::BenchReport,
    status::{Status, StatusKind},
    util::{IntoAdjustedByte, TryIntoAdjustedByte},
};

/// A text reporter for benchmark results.
pub struct TextReporter;

impl super::BenchReporter for TextReporter {
    fn print(&self, w: &mut dyn Write, report: &BenchReport) -> anyhow::Result<()> {
        print_summary(w, report)?;

        if report.stats.counter.iters > 0 {
            writeln!(w)?;
            print_latency(w, &report.hist)?;

            writeln!(w)?;
            print_status(w, &report.status_dist)?;
        }

        if !report.error_dist.is_empty() {
            writeln!(w)?;
            print_error(w, report)?;
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
    let counter = &report.stats.counter;

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
            format!("{}", counter.iters),
            format!("{:.2}/s", counter.iters as f64 / elapsed),
        ],
        vec![
            "Items".into(),
            format!("{}", counter.items),
            format!("{:.2}/s", counter.items as f64 / elapsed),
        ],
        vec![
            "Bytes".into(),
            format!("{:.2}", counter.bytes.adjusted()),
            format!("{:.2}/s", (counter.bytes as f64 / elapsed).adjusted()?),
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
        .sorted_unstable_by_key(|(_, &cnt)| Reverse(cnt))
        .collect_vec();
    writeln!(w, "{}", "Status distribution".h1())?;
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
        .sorted_unstable_by_key(|(_, &cnt)| Reverse(cnt))
        .collect_vec();
    let max = error_v.iter().map(|(_, iters)| iters).max().unwrap();
    let iters_width = max.to_string().len();
    writeln!(w, "{}", "Error distribution".h1())?;
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
