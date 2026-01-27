use anyhow::Result;
use crossterm::{
    ExecutableCommand, cursor,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{CompletedFrame, Frame, backend::CrosstermBackend};
use std::io;

pub(super) struct Terminal {
    terminal: ratatui::Terminal<CrosstermBackend<io::Stdout>>,
}

impl Terminal {
    pub(super) fn new() -> Result<Self> {
        enable_raw_mode()?;
        io::stdout().execute(cursor::Hide)?;
        io::stdout().execute(EnterAlternateScreen)?;

        Ok(Self {
            terminal: ratatui::Terminal::new(CrosstermBackend::new(io::stdout()))?,
        })
    }

    pub(super) fn draw<F>(&mut self, f: F) -> io::Result<CompletedFrame<'_>>
    where
        F: FnOnce(&mut Frame),
    {
        self.terminal.draw(f)
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = io::stdout().execute(LeaveAlternateScreen);
        let _ = io::stdout().execute(cursor::Show);
        let _ = disable_raw_mode();
    }
}
