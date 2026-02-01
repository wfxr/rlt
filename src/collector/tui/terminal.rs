use crate::error::TuiError;
use crossterm::{ExecutableCommand, cursor, terminal};
use ratatui::{CompletedFrame, Frame, backend::CrosstermBackend};
use std::io;

fn restore_terminal() {
    let _ = io::stdout().execute(terminal::LeaveAlternateScreen);
    let _ = io::stdout().execute(cursor::Show);
    let _ = terminal::disable_raw_mode();
}

pub(super) struct Terminal {
    terminal: ratatui::Terminal<CrosstermBackend<io::Stdout>>,
}

impl Terminal {
    pub(super) fn new() -> std::result::Result<Self, TuiError> {
        struct Guard {
            committed: bool,
        }

        impl Drop for Guard {
            fn drop(&mut self) {
                if !self.committed {
                    restore_terminal();
                }
            }
        }

        let mut g = Guard { committed: false };

        terminal::enable_raw_mode().map_err(TuiError::Init)?;

        io::stdout().execute(cursor::Hide).map_err(TuiError::Init)?;

        io::stdout().execute(terminal::EnterAlternateScreen).map_err(TuiError::Init)?;

        let terminal =
            ratatui::Terminal::new(CrosstermBackend::new(io::stdout())).map_err(TuiError::Init)?;

        g.committed = true;
        Ok(Self { terminal })
    }

    pub(super) fn draw<F>(&mut self, f: F) -> std::result::Result<CompletedFrame<'_>, TuiError>
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
