use crossterm::{ExecutableCommand, cursor, terminal};

use super::TuiResult;
use crate::error::TuiError;
use ratatui::{CompletedFrame, Frame, backend::CrosstermBackend};
use std::io;

/// Best-effort terminal restoration; errors are ignored since we may
/// already be in an error path and terminal state is uncertain.
fn restore_terminal() {
    let _ = io::stdout().execute(terminal::LeaveAlternateScreen);
    let _ = io::stdout().execute(cursor::Show);
    let _ = terminal::disable_raw_mode();
}

pub(super) struct Terminal {
    terminal: ratatui::Terminal<CrosstermBackend<io::Stdout>>,
}

impl Terminal {
    pub(super) fn new() -> TuiResult<Self> {
        terminal::enable_raw_mode().map_err(TuiError::Init)?;

        // From this point, restore terminal on failure
        let init = || {
            io::stdout().execute(cursor::Hide)?;
            io::stdout().execute(terminal::EnterAlternateScreen)?;
            ratatui::Terminal::new(CrosstermBackend::new(io::stdout()))
        };

        init().map(|terminal| Self { terminal }).map_err(|e| {
            restore_terminal();
            TuiError::Init(e)
        })
    }

    pub(super) fn draw<F>(&mut self, f: F) -> TuiResult<CompletedFrame<'_>>
    where
        F: FnOnce(&mut Frame),
    {
        self.terminal.draw(f).map_err(TuiError::Draw)
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        restore_terminal();
    }
}
