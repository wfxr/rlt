use crossterm::style::{StyledContent, Stylize};
use itertools::Itertools;
use std::{cmp::Reverse, io::Write};

use crate::{
    histogram::{LatencyHistogram, PERCENTAGES},
    report::BenchReport,
    status::StatusKind,
    util::{IntoAdjustedByte, TryIntoAdjustedByte},
};

pub struct TextReporter;

impl super::BenchReporter for TextReporter {
    fn print(&self, w: &mut dyn Write, report: &BenchReport) -> anyhow::Result<()> {
        print_summary(w, report)?;
        writeln!(w)?;

        if report.stats.counter.iters > 0 {
            print_latency(w, report)?;
            writeln!(w)?;

            print_status(w, report)?;
            writeln!(w)?;
        }

        print_error(w, report)?;

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

fn print_histogram(w: &mut dyn Write, hist: &LatencyHistogram, indent: usize) -> anyhow::Result<()> {
    let quantiles = hist
        .quantiles()
        .map(|(v, n)| (format!("{:.5}", v.as_secs_f64()), n))
        .collect_vec();
    if quantiles.is_empty() {
        return Ok(());
    }

    let max_count = quantiles.iter().map(|(_, n)| n).max().unwrap();
    let count_len = max_count.to_string().len();
    let value_len = quantiles.iter().map(|(v, _)| v.len()).max().unwrap();

    for (latency, count) in quantiles.iter() {
        write!(w, "{:indent$}", "")?;
        write!(w, "{}", &format!("  [{count:>count_len$}] {latency:>value_len$} |"),)?;
        let ratio = *count as f64 / *max_count as f64;
        // TODO: determine width dynamically
        let width = 32;
        for _ in 0..(width as f64 * ratio) as usize {
            write!(w, "■")?;
        }
        writeln!(w)?;
    }
    Ok(())
}

#[rustfmt::skip]
fn print_summary(w: &mut dyn Write, report: &BenchReport) -> anyhow::Result<()> {
    let elapsed = report.elapsed.as_secs_f64();
    let counter = &report.stats.counter;

    writeln!(w, "{}", "Summary".h1())?;
    writeln!(w,       "  Success ratio:  {}", render_success_ratio(100.0 * report.success_ratio()))?;
    writeln!(w,       "  Total time:     {:.3}s", elapsed)?;
    writeln!(w,       "  Concurrency:    {}", report.concurrency)?;
    writeln!(w, "{}", "  Iters".h2())?;
    writeln!(w,       "    Total:        {}", counter.iters)?;
    if counter.iters > 0 {
        writeln!(w,   "    Rate:         {:.2}", counter.iters as f64 / elapsed)?;
        writeln!(w,   "    Bytes/iter:   {:.2}", (counter.bytes as f64 / counter.iters as f64).to_bytes()?)?;
    }
    writeln!(w, "{}", "  Items".h2())?;
    writeln!(w,       "    Total:        {}", counter.items)?;
    if counter.items > 0 {
        writeln!(w,   "    Rate:         {:.2}", counter.items as f64 / elapsed)?;
        writeln!(w,   "    Items/iter:   {:.2}", counter.items as f64 / counter.iters as f64)?;
    }
    writeln!(w, "{}", "  Bytes".h2())?;
    writeln!(w,       "    Total:        {:.2}", counter.bytes.to_bytes())?;
    if counter.bytes > 0 {
        writeln!(w,   "    Rate:         {:.2}", (counter.bytes as f64 / elapsed).to_bytes()?)?;
    }
    Ok(())
}

#[rustfmt::skip]
fn print_latency(w: &mut dyn Write, report: &BenchReport) -> anyhow::Result<()> {
    writeln!(w, "{}", "Latency".h1())?;
    if report.hist.is_empty() {
        return Ok(());
    }

    writeln!(w, "{}", "  Stats".h2())?;
    writeln!(w,       "    Min:    {:.4}s", report.hist.min().as_secs_f64())?;
    writeln!(w,       "    Max:    {:.4}s", report.hist.max().as_secs_f64())?;
    writeln!(w,       "    Mean:   {:.4}s", report.hist.mean().as_secs_f64())?;
    writeln!(w,       "    Median: {:.4}s", report.hist.median().as_secs_f64())?;
    writeln!(w,       "    Stdev:  {:.4}s", report.hist.stdev() .as_secs_f64())?;
    writeln!(w)?;

    writeln!(w, "{}", "  Percentiles".h2())?;
    for (p, v) in report.hist.percentiles(PERCENTAGES) {
        writeln!(w,   "    {:.2}% in {:.4}s", p, v.as_secs_f64())?;
    }
    writeln!(w)?;

    writeln!(w, "{}", "  Histogram".h2())?;
    print_histogram(w, &report.hist, 2)?;

    Ok(())
}

fn print_status(w: &mut dyn Write, report: &BenchReport) -> anyhow::Result<()> {
    let status_v = report
        .status_dist
        .iter()
        .sorted_unstable_by_key(|(_, &cnt)| Reverse(cnt))
        .collect_vec();
    writeln!(w, "{}", "Status distribution".h1())?;
    if !status_v.is_empty() {
        let max = status_v.iter().map(|(_, iters)| iters).max().unwrap();
        let iters_width = max.to_string().len();
        for (&status, iters) in status_v {
            let line = format!("  [{iters:>iters_width$}] {status}");
            let line = match status.kind() {
                StatusKind::Success => line.green(),
                StatusKind::ClientError => line.yellow(),
                StatusKind::ServerError => line.red(),
                StatusKind::UnknownError => line.magenta(),
            };
            writeln!(w, "{line}")?;
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
    if !error_v.is_empty() {
        let max = error_v.iter().map(|(_, iters)| iters).max().unwrap();
        let iters_width = max.to_string().len();
        writeln!(w, "{}", "Error distribution".h1())?;
        for (error, count) in error_v {
            writeln!(w, "{}", format!("  [{count:>iters_width$}] {error}").red())?;
        }
    }
    Ok(())
}

trait ReportStyle {
    fn h1(&self) -> StyledContent<&str>;
    fn h2(&self) -> StyledContent<&str>;
}

impl<T: AsRef<str>> ReportStyle for T {
    fn h1(&self) -> StyledContent<&str> {
        self.as_ref().bold().underlined()
    }

    fn h2(&self) -> StyledContent<&str> {
        self.as_ref().bold()
    }
}
