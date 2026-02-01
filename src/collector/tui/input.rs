use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

use crate::error::TuiError;

use crate::phase::{BenchPhase, RunState};

use super::state::TimeWindowMode;

impl super::TuiCollector {
    /// Handle the user input events. Returns `true` if the collector should quit.
    pub(super) async fn handle_event(&mut self, elapsed: Duration) -> std::result::Result<bool, TuiError> {
        let clock = &mut self.bench_opts.clock;
        while crossterm::event::poll(Duration::from_secs(0)).map_err(TuiError::Poll)? {
            use KeyCode::*;
            if let Event::Key(KeyEvent { code, modifiers, .. }) =
                crossterm::event::read().map_err(TuiError::ReadEvent)?
            {
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
                    (Char('p') | Pause, _) => {
                        // Only mutate the clock once we're in the actual bench phase; setup/warmup
                        // runs with a paused clock by design.
                        let is_bench_phase = matches!(&*self.phase_rx.borrow(), BenchPhase::Bench);
                        match self.state.run_state {
                            RunState::Paused => {
                                if is_bench_phase {
                                    clock.resume();
                                }
                                self.state.run_state = RunState::Running;
                                self.pause.resume();
                            }
                            RunState::Running => {
                                if is_bench_phase {
                                    clock.pause();
                                }
                                self.state.run_state = RunState::Paused;
                                self.pause.pause();
                            }
                            RunState::Finished => {}
                        }
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
