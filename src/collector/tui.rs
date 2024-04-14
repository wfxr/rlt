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

cfg_if::cfg_if! {
    if #[cfg(feature = "log")] {
        use std::str::FromStr;
        use log::LevelFilter;
        use tui_logger::{TuiLoggerLevelOutput, TuiLoggerSmartWidget, TuiWidgetEvent, TuiWidgetState};
    }
}

use crate::{
    collector::ReportCollector,
    duration::DurationExt,
    histogram::{LatencyHistogram, PERCENTAGES},
    report::{BenchReport, IterReport},
    runner::BenchOpts,
    stats::{Counter, IterStats, RotateDiffWindowGroup, RotateWindowGroup},
    status::{Status, StatusKind},
    util::{IntoAdjustedByte, TryIntoAdjustedByte},
};

const SECOND: Duration = Duration::from_secs(1);

/// A report collector with real-time TUI support.
pub struct TuiCollector {
    /// The benchmark options.
    pub bench_opts: BenchOpts,
    /// Refresh rate for the tui collector, in frames per second (fps)
    pub fps: u8,
    /// The receiver for iteration reports.
    pub res_rx: mpsc::UnboundedReceiver<Result<IterReport>>,
    /// The sender for pausing the benchmark runner.
    pub pause: watch::Sender<bool>,
    /// The cancellation token for the benchmark runner.
    pub cancel: CancellationToken,

    #[cfg(feature = "log")]
    log_state: TuiWidgetState,
}

impl TuiCollector {
    /// Create a new TUI report collector.
    pub fn new(
        bench_opts: BenchOpts,
        fps: u8,
        res_rx: mpsc::UnboundedReceiver<Result<IterReport>>,
        pause: watch::Sender<bool>,
        cancel: CancellationToken,
    ) -> Result<Self> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "log")] {
                let log_level = match std::env::var("RUST_LOG") {
                    Ok(log_level) => LevelFilter::from_str(&log_level).unwrap_or(LevelFilter::Info),
                    Err(_) => LevelFilter::Info,
                };
                tui_logger::init_logger(log_level).map_err(|e| anyhow::anyhow!(e))?;
                tui_logger::set_default_level(log_level);
                let log_state = TuiWidgetState::new().set_default_display_level(log_level);
                Ok(Self { bench_opts, fps, res_rx, pause, cancel, log_state })
            } else {
                Ok(Self { bench_opts, fps, res_rx, pause, cancel })
            }
        }
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

        let mut clock = self.bench_opts.clock.clone();

        let mut latest_iters = RotateWindowGroup::new(60);
        let mut latest_iters_ticker = clock.ticker(SECOND);

        let mut latest_stats = RotateDiffWindowGroup::new(self.fps);
        let mut latest_stats_ticker = clock.ticker(SECOND / self.fps as u32);

        let mut ui_ticker = tokio::time::interval(SECOND / self.fps as u32);
        ui_ticker.set_missed_tick_behavior(MissedTickBehavior::Burst);

        #[cfg(feature = "log")]
        let mut show_logs = false;

        let mut elapsed;
        'outer: loop {
            loop {
                tokio::select! {
                    biased;
                    _ = ui_ticker.tick() => {
                        while crossterm::event::poll(Duration::from_secs(0))? {
                            use KeyCode::*;
                            if let Event::Key(KeyEvent { code, modifiers, .. }) = crossterm::event::read()? {
                                match (code, modifiers) {
                                    (Char('+'), _) => {
                                        current_tw = current_tw.prev();
                                        auto_tw = false;
                                    }
                                    (Char('-'), _) => {
                                        current_tw = current_tw.next();
                                        auto_tw = false;
                                    }
                                    (Char('a'), _) => auto_tw = true,
                                    (Char('q'), _) | (Char('c'), KeyModifiers::CONTROL) => {
                                        self.cancel.cancel();
                                        break 'outer;
                                    }
                                    (Char('p') | Pause, _) => {
                                        let pause = !*self.pause.borrow();
                                        if pause {
                                            clock.pause();
                                        } else {
                                            clock.resume();
                                        }
                                        self.pause.send_replace(pause);
                                    }
                                    #[cfg(feature = "log")]
                                    (Char('l'), _) => show_logs = !show_logs,
                                    #[cfg(feature = "log")]
                                    (code, _) if show_logs => {
                                        use TuiWidgetEvent::*;
                                        let mut txn = |e| self.log_state.transition(e);
                                        match code {
                                            Char(' ')            => txn(HideKey),
                                            PageDown | Char('f') => txn(NextPageKey),
                                            PageUp   | Char('b') => txn(PrevPageKey),
                                            Up                   => txn(UpKey),
                                            Down                 => txn(DownKey),
                                            Left                 => txn(LeftKey),
                                            Right                => txn(RightKey),
                                            Enter                => txn(FocusKey),
                                            Esc                  => txn(EscapeKey),
                                            _                    => (),
                                        }
                                    }
                                    _ => (),
                                }
                            }
                        }

                        elapsed = clock.elapsed();
                        current_tw = if auto_tw {
                            *TimeWindow::variants().iter().rfind(|&&ts| elapsed > ts.into()).unwrap_or(&TimeWindow::Second)
                        } else {
                            current_tw
                        };
                        break;
                    }
                    _ = latest_stats_ticker.tick() => {
                        latest_stats.rotate(&stats);
                        continue;
                    }
                    _ = latest_iters_ticker.tick() => {
                        latest_iters.rotate();
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

                #[cfg(feature = "log")]
                if show_logs {
                    tui_log::render_logs(f, &self.log_state);
                }
            })?;
        }

        let elapsed = clock.elapsed();
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
            "Stats for ".into(),
            format!("last {}", tw).yellow().bold(),
        ])),
        &stats.counter,
        duration,
    );
}

fn render_stats_overall(frame: &mut Frame, area: Rect, counter: &Counter, elapsed: Duration) {
    render_stats(frame, area, "Stats overall".into(), counter, elapsed);
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
        .block(Block::new().title("Progress").borders(Borders::ALL))
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
    let p = Paragraph::new(dist).block(Block::new().title("Status distribution").borders(Borders::ALL));
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

    let data: Vec<(&str, u64)> = quantiles.iter().map(|(d, n)| (d.as_str(), *n)).collect();
    let chart = BarChart::default()
        .block(
            Block::new()
                .title(Title::from(Line::from(vec![
                    "Latency histogram (".into(),
                    u.to_string().yellow().bold(),
                    ")".into(),
                ])))
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

    let area = area.inner(&Margin::new(1, 1));
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

fn gen_tips<'a>(tips: impl IntoIterator<Item = (&'a str, &'a str)>) -> Line<'a> {
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
        #[cfg(feature = "log")]
        ("l", "Logs window"),
        ("p", "Pause"),
        ("q", "Quit"),
    ])
    .right_aligned();
    frame.render_widget(tips, area.inner(&Margin::new(1, 0)));
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

#[cfg(feature = "log")]
mod tui_log {
    use super::*;

    #[cfg(feature = "log")]
    pub(crate) fn render_logs(frame: &mut Frame, log_state: &TuiWidgetState) {
        let log_widget = TuiLoggerSmartWidget::default()
            .style_error(Style::default().fg(Color::Red))
            .style_debug(Style::default().fg(Color::Green))
            .style_warn(Style::default().fg(Color::Yellow))
            .style_trace(Style::default().fg(Color::Magenta))
            .style_info(Style::default().fg(Color::Cyan))
            .border_type(ratatui::widgets::BorderType::Rounded)
            .output_separator('|')
            .output_level(Some(TuiLoggerLevelOutput::Abbreviated))
            .output_target(true)
            .output_file(true)
            .output_line(true)
            .title_log("Logs")
            .title_target("Selector")
            .state(log_state);

        let area = centered_rect(80, 80, frame.size());
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100), Constraint::Min(1)])
            .split(area.inner(&Margin::new(1, 1)));
        let tips = gen_tips([
            ("Enter", "Focus target"),
            ("↑/↓", "Select target"),
            ("←/→", "Display level"),
            ("f/b", "Scroll"),
            ("Esc", "Cancel scroll"),
            ("Space", "Hide selector"),
        ])
        .right_aligned();

        frame.render_widget(Clear, area);
        frame.render_widget(log_widget, rows[0]);
        frame.render_widget(tips, rows[1].inner(&Margin::new(1, 0)));
    }

    #[cfg(feature = "log")]
    pub(crate) fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::vertical([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

        Layout::horizontal([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
    }
}
