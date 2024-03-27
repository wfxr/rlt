use anyhow::Result;
use async_trait::async_trait;
use crossterm::{
    cursor,
    event::{Event, KeyCode, KeyEvent, KeyModifiers},
    terminal, ExecutableCommand,
};
use itertools::Itertools;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{block::Title, BarChart, Block, Borders, Clear, Gauge, Padding, Paragraph},
    CompletedFrame, Frame,
};
use std::{collections::HashMap, fmt, io, time::Duration};
use tokio::{
    sync::{mpsc, watch},
    time::MissedTickBehavior,
};
use tokio_util::sync::CancellationToken;

use crate::{
    collector::ReportCollector,
    histogram::{LatencyHistogram, PERCENTAGES},
    report::{BenchReport, IterReport},
    runner::BenchOpts,
    stats::{Counter, IterStats, RotateDiffWindowGroup, RotateWindowGroup},
    status::{Status, StatusKind},
    util::{IntoAdjustedByte, TryIntoAdjustedByte},
};

pub struct TuiCollector {
    pub bench_opts: BenchOpts,
    pub fps: u8,
    pub res_rx: mpsc::UnboundedReceiver<Result<IterReport>>,
    pub pause: watch::Sender<bool>,
    pub cancel: CancellationToken,
}

impl TuiCollector {
    pub fn new(
        bench_opts: BenchOpts,
        fps: u8,
        res_rx: mpsc::UnboundedReceiver<Result<IterReport>>,
        pause: watch::Sender<bool>,
        cancel: CancellationToken,
    ) -> Self {
        Self { bench_opts, fps, res_rx, pause, cancel }
    }
}

struct Terminal {
    terminal: ratatui::Terminal<CrosstermBackend<io::Stdout>>,
}
impl Terminal {
    fn new() -> Result<Self> {
        crossterm::terminal::enable_raw_mode()?;
        io::stdout().execute(crossterm::cursor::Hide)?;
        io::stdout().execute(crossterm::terminal::EnterAlternateScreen)?;

        Ok(Self {
            terminal: ratatui::Terminal::new(CrosstermBackend::new(io::stdout()))?,
        })
    }

    fn draw<F>(&mut self, f: F) -> io::Result<CompletedFrame>
    where
        F: FnOnce(&mut Frame),
    {
        self.terminal.draw(f)
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        std::io::stdout().execute(terminal::LeaveAlternateScreen).unwrap();
        std::io::stdout().execute(cursor::Show).unwrap();
        crossterm::terminal::disable_raw_mode().unwrap();
    }
}

#[async_trait]
impl ReportCollector for TuiCollector {
    async fn run(&mut self) -> Result<BenchReport> {
        let mut terminal = Terminal::new()?;

        let mut hist = LatencyHistogram::new();
        let mut stats = IterStats::new();
        let mut status_dist = HashMap::new();
        let mut error_dist = HashMap::new();

        let mut current_tw = TimeWindow::Second;
        let mut auto_tw = true;

        let start = self.bench_opts.start;

        let mut latest_iters = RotateWindowGroup::new(start, 60);
        const SECOND: Duration = Duration::from_secs(1);
        let mut latest_iters_timer = tokio::time::interval_at(start + SECOND, SECOND);
        latest_iters_timer.set_missed_tick_behavior(MissedTickBehavior::Burst);

        let mut latest_stats = RotateDiffWindowGroup::new(start, self.fps);
        let mut refresh_timer = tokio::time::interval(Duration::from_secs(1) / self.fps as u32);
        refresh_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let mut elapsed;
        'outer: loop {
            loop {
                tokio::select! {
                    biased;
                    t = refresh_timer.tick() => {
                        latest_stats.rotate(t, &stats);

                        while crossterm::event::poll(Duration::from_secs(0))? {
                            match crossterm::event::read()? {
                                Event::Key(KeyEvent { code: KeyCode::Char('+'), .. }) => {
                                    current_tw = current_tw.prev();
                                    auto_tw = false;
                                }
                                Event::Key(KeyEvent { code: KeyCode::Char('-'), .. }) => {
                                    current_tw = current_tw.next();
                                    auto_tw = false;
                                }
                                Event::Key(KeyEvent { code: KeyCode::Char('a'), .. }) => auto_tw = true,
                                Event::Key(KeyEvent { code: KeyCode::Char('q'), .. })
                                | Event::Key(KeyEvent {
                                    code: KeyCode::Char('c'), modifiers: KeyModifiers::CONTROL, ..
                                }) => {
                                    self.cancel.cancel();
                                    break;
                                }
                                // TODO: pause logical time instead of real time
                                Event::Key(KeyEvent { code: KeyCode::Char('p'), .. }) | Event::Key(KeyEvent { code: KeyCode::Pause, .. }) => {
                                    let pause = !*self.pause.borrow();
                                    self.pause.send_replace(pause);
                                }
                                _ => (),
                            }
                        }

                        elapsed = t - start;
                        current_tw = if auto_tw && !*self.pause.borrow() {
                            *TimeWindow::variants().iter().rfind(|&&ts| elapsed > ts.into()).unwrap_or(&TimeWindow::Second)
                        } else {
                            current_tw
                        };
                        break;
                    }
                    t = latest_iters_timer.tick() => {
                        latest_iters.rotate(t);
                        continue;
                    }
                    r = self.res_rx.recv() => match r {
                        Some(Ok(report)) => {
                            *status_dist.entry(report.status).or_default() += 1;
                            hist.record(report.duration)?;
                            latest_iters.push(&report);
                            stats += &report;
                        }
                        Some(Err(e)) => *error_dist.entry(e.to_string()).or_default() += 1,
                        None => break 'outer,
                    }
                };
            }

            terminal.draw(|f| {
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
                    .split(f.size());

                let mid = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(50),
                        Constraint::Percentage(50),
                        Constraint::Percentage(50),
                    ])
                    .split(rows[0]);

                let bot = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                    .split(rows[2]);

                let paused = *self.pause.borrow();
                render_process_gauge(f, rows[3], &stats.counter, elapsed, &self.bench_opts, paused);
                render_stats_overall(f, mid[1], &stats.counter, elapsed);
                render_stats_timewin(f, mid[0], &latest_stats, current_tw);
                render_status_dist(f, mid[2], &status_dist);
                render_error_dist(f, rows[1], &error_dist);
                render_iter_hist(f, bot[0], &latest_iters, current_tw);
                render_latency_hist(f, bot[1], &hist, 7);
                render_tips(f, rows[4]);
            })?;
        }

        let elapsed = start.elapsed();
        let concurrency = self.bench_opts.concurrency;
        Ok(BenchReport { concurrency, hist, stats, status_dist, error_dist, elapsed })
    }
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
        Title::from(Line::from(vec![
            " Stats for ".into(),
            format!("last {} ", tw).yellow().bold(),
        ])),
        &stats.counter,
        duration,
    );
}

fn render_stats_overall(frame: &mut Frame, area: Rect, counter: &Counter, elapsed: Duration) {
    render_stats(frame, area, " Stats overall ".into(), counter, elapsed);
}

fn render_stats(frame: &mut Frame, area: Rect, title: Title, counter: &Counter, elapsed: Duration) {
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
        Line::from(format!("Items: {}", counter.items)),
        Line::from(format!("Iters: {}", counter.iters)),
        Line::from(format!("Bytes: {:.2}", counter.bytes.to_bytes())),
    ];
    Paragraph::new(lines).block(Block::new().borders(Borders::NONE))
}

fn render_stats_rate(counter: &Counter, elapsed: Duration) -> Paragraph<'static> {
    let secs = elapsed.as_secs_f64();
    let lines = vec![
        Line::from(format!("{:.2} iters/s", counter.iters as f64 / secs)),
        Line::from(format!("{:.2} items/s", counter.items as f64 / secs)),
        Line::from(format!(
            "{}/s",
            match (counter.bytes as f64 / secs).to_bytes() {
                Ok(bps) => format!("{:.2}", bps),
                Err(_) => "NaN B".to_string(),
            }
        )),
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

    if paused {
        label.push_str(" (PAUSED)");
    }

    let guage = Gauge::default()
        .block(Block::new().title(" Progress ").borders(Borders::ALL))
        .gauge_style(Style::new().fg(Color::Cyan))
        .label(label)
        .ratio(progress);
    frame.render_widget(guage, area);
}

fn render_status_dist(frame: &mut Frame, area: Rect, status_dist: &HashMap<Status, u64>) {
    let dist = status_dist
        .iter()
        .sorted_by_key(|(_, &cnt)| std::cmp::Reverse(cnt))
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
    let p = Paragraph::new(dist).block(Block::new().title(" Status code distribution").borders(Borders::ALL));
    frame.render_widget(p, area);
}

fn render_error_dist(frame: &mut Frame, area: Rect, error_dist: &HashMap<String, u64>) {
    if error_dist.is_empty() {
        return;
    }

    let dist = error_dist
        .iter()
        .sorted_by_key(|(_, &cnt)| std::cmp::Reverse(cnt))
        .map(|(err, cnt)| Line::from(format!("[{cnt}] {err}")))
        .collect_vec();
    let p = Paragraph::new(dist).block(Block::new().title(" Error distribution ").borders(Borders::ALL));
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
    let data: Vec<(String, u64)> = win
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
        .collect();

    let bar_num_iter_str: Vec<(&str, u64)> = data.iter().map(|(a, b)| (a.as_str(), *b)).collect();
    let bar_width = data
        .iter()
        .map(|(s, _)| s.chars().count())
        .max()
        .map(|w| w + 2)
        .unwrap_or(1) as u16;
    let chart = BarChart::default()
        .block(
            Block::new()
                .title(" Iteration histogram ")
                .style(Style::new().fg(Color::Green).bg(Color::Reset))
                .borders(Borders::ALL),
        )
        .data(bar_num_iter_str.as_slice())
        .bar_width(bar_width)
        .to_owned();
    frame.render_widget(chart, area);
}

fn render_latency_hist(frame: &mut Frame, area: Rect, hist: &LatencyHistogram, histo_width: usize) {
    let quantiles = hist
        .quantiles()
        .map(|(l, v)| (l.as_secs_f64().to_string(), v))
        .collect_vec();

    let data: Vec<(&str, u64)> = quantiles.iter().map(|(l, v)| (l.as_str(), *v)).collect();
    let chart = BarChart::default()
        .block(
            Block::new()
                .title(" Latency histogram")
                .style(Style::new().fg(Color::Yellow).bg(Color::Reset))
                .borders(Borders::ALL),
        )
        .data(&data)
        .bar_width(histo_width as u16);
    frame.render_widget(chart, area);

    if hist.is_empty() {
        return;
    }

    let area = area.inner(&Margin::new(1, 1));
    let area = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);
    let area = Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area[1]);
    let area = area[0];

    let mut content = vec![
        format!("Max: {:.4}s", hist.max().as_secs_f64()),
        format!("Min: {:.4}s", hist.min().as_secs_f64()),
        format!("Mean: {:.4}s", hist.mean().as_secs_f64()),
        format!("Median: {:.4}s", hist.median().as_secs_f64()),
        format!("StdDev: {:.4}s", hist.stdev().as_secs_f64()),
    ];
    content.push("".to_string());
    content.extend(
        hist.percentiles(PERCENTAGES)
            .map(|(p, v)| format!("p{:.2}% in {:.4}s", p, v.as_secs_f64())),
    );
    let width = content.iter().map(|s| s.len()).max().unwrap_or(0) + 2;
    if width > area.width as usize {
        return;
    }
    let area = Rect {
        x: area.x + area.width - width as u16,
        y: area.y,
        width: width as u16,
        height: content.len() as u16,
    };
    let content = content.into_iter().map(Line::from).collect_vec();
    let block = Block::default().padding(Padding::right(2)).borders(Borders::NONE);
    let paragraph = Paragraph::new(content).block(block).right_aligned();

    frame.render_widget(Clear, area); //clears out the background
    frame.render_widget(paragraph, area);
}

fn render_tips(frame: &mut Frame, area: Rect) {
    // TODO: is there a better way to render this?
    let tips = vec![Line::from(vec![
        "Press ".italic(),
        "-".bold().yellow(),
        "/".italic(),
        "+".bold().yellow(),
        " to zoom in/out the time window, ".italic(),
        "a".bold().yellow(),
        " to enable auto time window, ".italic(),
        "p".bold().yellow(),
        " to pause, ".italic(),
        "q".bold().yellow(),
        " to quit ".italic(),
    ])];
    let p = Paragraph::new(tips)
        .block(Block::default().borders(Borders::NONE))
        .right_aligned();
    frame.render_widget(p, area);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimeWindow {
    Second = 1,
    TenSec = 10,
    Minute = 60,
    TenMin = 600,
}

impl TimeWindow {
    fn variants() -> &'static [TimeWindow] {
        use TimeWindow::*;
        &[Second, TenSec, Minute, TenMin]
    }
}

impl fmt::Display for TimeWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", humantime::format_duration(Duration::from(*self)))
    }
}

impl From<TimeWindow> for Duration {
    fn from(tw: TimeWindow) -> Self {
        Duration::from_secs(tw as u64)
    }
}

impl TimeWindow {
    pub fn format(&self, n: usize) -> String {
        match self {
            TimeWindow::Second => format!("{}s", n),
            TimeWindow::TenSec => format!("{}s", 10 * n),
            TimeWindow::Minute => format!("{}m", n),
            TimeWindow::TenMin => format!("{}m", 10 * n),
        }
    }

    pub fn next(&self) -> Self {
        match self {
            TimeWindow::Second => TimeWindow::TenSec,
            TimeWindow::TenSec => TimeWindow::Minute,
            TimeWindow::Minute => TimeWindow::TenMin,
            TimeWindow::TenMin => TimeWindow::TenMin,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            TimeWindow::Second => TimeWindow::Second,
            TimeWindow::TenSec => TimeWindow::Second,
            TimeWindow::Minute => TimeWindow::TenSec,
            TimeWindow::TenMin => TimeWindow::Minute,
        }
    }
}
