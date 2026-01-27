use itertools::Itertools;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{BarChart, Block, Borders, Clear, Gauge, Padding, Paragraph},
};
use std::{collections::HashMap, time::Duration};

use crate::{
    duration::DurationExt,
    histogram::{LatencyHistogram, PERCENTAGES},
    runner::BenchOpts,
    stats::{Counter, RotateDiffWindowGroup, RotateWindowGroup},
    status::{Status, StatusKind},
    util::{IntoAdjustedByte, TryIntoAdjustedByte},
};

use super::state::TimeWindow;

#[allow(clippy::too_many_arguments)]
pub(super) fn render_dashboard(
    frame: &mut Frame,
    counter: &Counter,
    elapsed: Duration,
    opts: &BenchOpts,
    paused: bool,
    finished: bool,
    latest_stats: &RotateDiffWindowGroup,
    tw: TimeWindow,
    status_dist: &HashMap<Status, u64>,
    error_dist: &HashMap<String, u64>,
    latest_iters: &RotateWindowGroup,
    hist: &LatencyHistogram,
) {
    let progress_height = 3;
    let stats_height = 5;
    let error_dist_height = match error_dist.len() {
        0 => 0,
        len => len.min(5) as u16 + 2,
    };
    let hist_height_filler = 40;
    let tips_height = 1;
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(stats_height),
            Constraint::Length(error_dist_height),
            Constraint::Fill(hist_height_filler),
            Constraint::Length(progress_height),
            Constraint::Length(tips_height),
        ])
        .split(frame.area());

    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Fill(1), Constraint::Fill(1)])
        .split(rows[0]);

    let bot = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(rows[2]);

    render_process_gauge(frame, rows[3], counter, elapsed, opts, paused, finished);
    render_stats_overall(frame, mid[1], counter, elapsed);
    render_stats_timewin(frame, mid[0], latest_stats, tw);
    render_status_dist(frame, mid[2], status_dist);
    render_error_dist(frame, rows[1], error_dist);
    render_iter_hist(frame, bot[0], latest_iters, tw);
    render_latency_hist(frame, bot[1], hist, 7);
    render_tips(frame, rows[4]);
}

fn render_stats_timewin(frame: &mut Frame, area: Rect, stats: &RotateDiffWindowGroup, tw: TimeWindow) {
    let (stats, duration) = match tw {
        TimeWindow::Second => stats.stats_last_sec(),
        TimeWindow::TenSec => stats.stats_last_10sec(),
        TimeWindow::Minute => stats.stats_last_min(),
        TimeWindow::TenMin => stats.stats_last_10min(),
    };

    render_stats(
        frame,
        area,
        Line::from(vec!["Stats for ".into(), format!("last {}", tw).yellow().bold()]),
        &stats.counter,
        duration,
    );
}

fn render_stats_overall(frame: &mut Frame, area: Rect, counter: &Counter, elapsed: Duration) {
    render_stats(frame, area, "Stats overall".into(), counter, elapsed);
}

fn render_stats(frame: &mut Frame, area: Rect, title: Line<'_>, counter: &Counter, elapsed: Duration) {
    let block = Block::new().title(title).borders(Borders::ALL);

    let [lhs, rhs] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(block.inner(area));

    let stats_counter = render_stats_counter(counter);
    let stats_rate = render_stats_rate(counter, elapsed);

    frame.render_widget(stats_counter, lhs);
    frame.render_widget(stats_rate, rhs);
    frame.render_widget(block, area);
}

fn render_stats_counter(counter: &Counter) -> Paragraph<'static> {
    let lines = vec![
        Line::from(vec!["Items: ".into(), counter.items.to_string().green()]),
        Line::from(vec!["Iters: ".into(), counter.iters.to_string().green()]),
        Line::from(vec![
            "Bytes: ".into(),
            format!("{:.2}", counter.bytes.adjusted()).green(),
        ]),
    ];
    Paragraph::new(lines).block(Block::new().borders(Borders::NONE))
}

fn render_stats_rate(counter: &Counter, elapsed: Duration) -> Paragraph<'static> {
    let secs = elapsed.as_secs_f64();
    let lines = vec![
        Line::from(format!("{:.2} iters/s", counter.iters as f64 / secs).green()),
        Line::from(format!("{:.2} items/s", counter.items as f64 / secs).green()),
        Line::from(
            format!(
                "{}/s",
                match (counter.bytes as f64 / secs).adjusted() {
                    Ok(bps) => format!("{:.2}", bps),
                    Err(_) => "NaN B".to_string(),
                }
            )
            .green(),
        ),
    ];
    Paragraph::new(lines).block(Block::new().borders(Borders::NONE))
}

fn render_process_gauge(
    frame: &mut Frame,
    area: Rect,
    counter: &Counter,
    elapsed: Duration,
    opts: &BenchOpts,
    paused: bool,
    finished: bool,
) {
    let rounded = |duration: Duration| humantime::Duration::from(Duration::from_secs(duration.as_secs_f64() as u64));
    let time_progress = |duration: &Duration| {
        (
            (elapsed.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0),
            format!("{} / {}", rounded(elapsed), rounded(*duration)),
        )
    };
    let iter_progress = |iters: &u64| {
        (
            (counter.iters as f64 / *iters as f64).clamp(0.0, 1.0),
            format!("{} / {}", counter.iters, iters),
        )
    };

    let (progress, mut label) = match opts {
        BenchOpts { duration: None, iterations: None, .. } => (0.0, "INFINITE".to_string()),
        BenchOpts { duration: Some(duration), iterations: None, .. } => time_progress(duration),
        BenchOpts { duration: None, iterations: Some(iters), .. } => iter_progress(iters),
        BenchOpts { duration: Some(duration), iterations: Some(iters), .. } => {
            let iter_ratio = counter.iters as f64 / *iters as f64;
            let time_ratio = elapsed.as_secs_f64() / duration.as_secs_f64();
            if iter_ratio > time_ratio {
                iter_progress(iters)
            } else {
                time_progress(duration)
            }
        }
    };

    let style = match (finished, paused) {
        (true, _) => {
            label.push_str(" (FINISHED)");
            Style::new().fg(Color::Yellow)
        }
        (_, true) => {
            label.push_str(" (PAUSED)");
            Style::new().fg(Color::Yellow)
        }
        (false, false) => Style::new().fg(Color::Cyan),
    };

    let guage = Gauge::default()
        .block(Block::new().title("Progress").borders(Borders::ALL))
        .gauge_style(style)
        .label(label)
        .ratio(progress);
    frame.render_widget(guage, area);
}

fn render_status_dist(frame: &mut Frame, area: Rect, status_dist: &HashMap<Status, u64>) {
    let dist = status_dist
        .iter()
        .sorted_by_key(|&(_, cnt)| std::cmp::Reverse(cnt))
        .map(|(status, cnt)| {
            let s = format!("{} {} iters", status, cnt);
            let s = match status.kind() {
                StatusKind::Success => s.green(),
                StatusKind::ClientError => s.yellow(),
                StatusKind::ServerError => s.red(),
                StatusKind::Error => s.magenta(),
            };
            Line::from(s)
        })
        .collect_vec();
    let p = Paragraph::new(dist).block(Block::new().title("Status distribution").borders(Borders::ALL));
    frame.render_widget(p, area);
}

fn render_error_dist(frame: &mut Frame, area: Rect, error_dist: &HashMap<String, u64>) {
    if error_dist.is_empty() {
        return;
    }

    let dist = error_dist
        .iter()
        .sorted_by_key(|&(_, cnt)| std::cmp::Reverse(cnt))
        .map(|(err, cnt)| Line::from(format!("[{cnt}] {err}")))
        .collect_vec();
    let p = Paragraph::new(dist).block(Block::new().title("Error distribution").borders(Borders::ALL));
    frame.render_widget(p, area);
}

fn render_iter_hist(frame: &mut Frame, area: Rect, rwg: &RotateWindowGroup, tw: TimeWindow) {
    let win = match tw {
        TimeWindow::Second => &rwg.stats_by_sec,
        TimeWindow::TenSec => &rwg.stats_by_10sec,
        TimeWindow::Minute => &rwg.stats_by_min,
        TimeWindow::TenMin => &rwg.stats_by_10min,
    };
    let cols = win.iter().map(|w| w.counter.iters.to_string().len()).max().unwrap_or(0);
    let data = win
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let mut s = tw.format(i);
            if cols > s.len() {
                for _ in 0..cols - s.len() {
                    s.push(' ');
                }
            }
            (s, n.counter.iters)
        })
        .collect_vec();

    let bar_num_iter_str = data.iter().map(|(a, b)| (a.as_str(), *b)).collect_vec();
    let bar_width = data
        .iter()
        .map(|(s, _)| s.chars().count())
        .max()
        .map(|w| w + 2)
        .unwrap_or(1) as u16;
    let chart = BarChart::default()
        .block(Block::new().title("Iteration histogram").borders(Borders::ALL))
        .data(bar_num_iter_str.as_slice())
        .bar_style(Style::default().fg(Color::Green))
        .label_style(Style::default().fg(Color::Cyan))
        .bar_width(bar_width);
    frame.render_widget(chart, area);
}

fn render_latency_hist(frame: &mut Frame, area: Rect, hist: &LatencyHistogram, histo_width: usize) {
    // time unit for the histogram
    let u = hist.median().appropriate_unit();

    let quantiles = hist
        .quantiles()
        .map(|(d, n)| (d.as_f64(u).to_string(), n))
        .collect_vec();

    let data = quantiles.iter().map(|(d, n)| (d.as_str(), *n)).collect_vec();
    let chart = BarChart::default()
        .block(
            Block::new()
                .title(Line::from(vec![
                    "Latency histogram (".into(),
                    u.to_string().yellow().bold(),
                    ")".into(),
                ]))
                .borders(Borders::ALL),
        )
        .data(&data)
        .bar_style(Style::default().fg(Color::Green))
        .label_style(Style::default().fg(Color::Cyan))
        .bar_width(histo_width as u16);
    frame.render_widget(chart, area);

    if hist.is_empty() {
        return;
    }

    let area = area.inner(Margin::new(1, 1));
    let area = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);
    let area = Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area[1]);
    let area = area[0];

    // max width of the formatted duration
    let w = format!("{:.2}", hist.max().as_f64(u)).len();
    #[rustfmt::skip]
    let mut content = vec![
        Line::from(vec!["Avg: ".blue(),      format!("{: >w$.2}", hist.mean().as_f64(u)).green()]),
        Line::from(vec!["Min: ".cyan(),      format!("{: >w$.2}", hist.min().as_f64(u)).green()]),
        Line::from(vec!["Med: ".yellow(),    format!("{: >w$.2}", hist.median().as_f64(u)).green()]),
        Line::from(vec!["Max: ".red(),       format!("{: >w$.2}", hist.max().as_f64(u)).green()]),
        Line::from(vec!["Stdev: ".magenta(), format!("{: >w$.2}", hist.stdev().as_f64(u)).green()]),
    ];
    content.push(Line::default());

    content.extend(hist.percentiles(PERCENTAGES).map(|(p, d)| {
        Line::from(vec![
            format!("P{:.2}%: ", p).cyan(),
            format!("{: >w$.2}", d.as_f64(u)).green(),
        ])
    }));
    let width = content.iter().map(|s| s.width()).max().unwrap_or(0) + 2;
    if width > area.width as usize {
        return;
    }
    let area = Rect {
        x: area.x + area.width - width as u16,
        y: area.y,
        width: width as u16,
        height: content.len() as u16,
    };
    let block = Block::default().padding(Padding::right(2)).borders(Borders::NONE);
    let paragraph = Paragraph::new(content).block(block).right_aligned();

    frame.render_widget(Clear, area); //clears out the background
    frame.render_widget(paragraph, area);
}

pub(super) fn gen_tips<'a>(tips: impl IntoIterator<Item = (&'a str, &'a str)>) -> Line<'a> {
    #[allow(unstable_name_collisions)]
    tips.into_iter()
        .map(|(key, tip)| vec![key.bold().yellow().italic(), ": ".into(), tip.italic()])
        .intersperse(vec![", ".into()])
        .flatten()
        .collect_vec()
        .into()
}

fn render_tips(frame: &mut Frame, area: Rect) {
    let tips = gen_tips([
        ("+/-", "Zoom in/out"),
        ("a", "Auto time window"),
        #[cfg(feature = "tracing")]
        ("l", "Logs window"),
        ("p", "Pause"),
        ("q", "Quit"),
    ])
    .right_aligned();
    frame.render_widget(tips, area.inner(Margin::new(1, 0)));
}
