use anyhow::Result;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style},
    widgets::Clear,
};

use log::LevelFilter;
use tui_logger::{TuiLoggerLevelOutput, TuiLoggerSmartWidget, TuiWidgetState};

pub(crate) struct LogState {
    pub(crate) inner: TuiWidgetState,
    pub(crate) display: bool,
}

impl LogState {
    pub(crate) fn from_env() -> Result<Self> {
        tui_logger::set_default_level(LevelFilter::Trace);
        let state = TuiWidgetState::new().set_default_display_level(LevelFilter::Info);
        Ok(Self { inner: state, display: false })
    }
}

pub(crate) fn render_logs(frame: &mut Frame, state: &LogState) {
    if !state.display {
        return;
    }

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
        .state(&state.inner);

    let area = centered_rect(80, 80, frame.area());
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(100), Constraint::Min(1)])
        .split(area.inner(Margin::new(1, 1)));
    let tips = super::render::gen_tips([
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
    frame.render_widget(tips, rows[1].inner(Margin::new(1, 0)));
}

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
