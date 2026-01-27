use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

use super::state::TimeWindowMode;

impl super::TuiCollector {
    /// Handle the user input events. Returns `true` if the collector should quit.
    pub(super) async fn handle_event(&mut self, elapsed: Duration) -> Result<bool> {
        let clock = &mut self.bench_opts.clock;
        while crossterm::event::poll(Duration::from_secs(0))? {
            use KeyCode::*;
            if let Event::Key(KeyEvent { code, modifiers, .. }) = crossterm::event::read()? {
                match (code, modifiers) {
                    (Char('+'), _) => {
                        let tw = self.state.tm_win.effective(elapsed);
                        self.state.tm_win = TimeWindowMode::Manual(tw.prev());
                    }
                    (Char('-'), _) => {
                        let tw = self.state.tm_win.effective(elapsed);
                        self.state.tm_win = TimeWindowMode::Manual(tw.next());
                    }
                    (Char('a'), _) => {
                        self.state.tm_win = TimeWindowMode::Auto;
                    }
                    (Char('q'), _) | (Char('c'), KeyModifiers::CONTROL) => {
                        self.cancel.cancel();
                        return Ok(true);
                    }
                    (Char('p') | Pause, _) if !self.state.finished => {
                        let pause = !*self.pause.borrow();
                        if pause {
                            clock.pause();
                        } else {
                            clock.resume();
                        }
                        self.pause.send_replace(pause);
                    }
                    #[cfg(feature = "tracing")]
                    (Char('l'), _) => self.state.log.display = !self.state.log.display,
                    #[cfg(feature = "tracing")]
                    (code, _) if self.state.log.display => {
                        use tui_logger::TuiWidgetEvent::*;
                        let txn = |e| self.state.log.inner.transition(e);
                        match code {
                            Char(' ') => txn(HideKey),
                            PageDown | Char('f') => txn(NextPageKey),
                            PageUp | Char('b') => txn(PrevPageKey),
                            Up => txn(UpKey),
                            Down => txn(DownKey),
                            Left => txn(LeftKey),
                            Right => txn(RightKey),
                            Enter => txn(FocusKey),
                            Esc => txn(EscapeKey),
                            _ => (),
                        }
                    }
                    _ => (),
                }
            }
        }
        Ok(false)
    }
}
