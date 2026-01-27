use std::{fmt, time::Duration};

pub(super) struct TuiCollectorState {
    pub(super) tm_win: TimeWindow,
    pub(super) finished: bool,
    #[cfg(feature = "tracing")]
    pub(super) log: super::tui_log::LogState,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimeWindow {
    Second = 1,
    TenSec = 10,
    Minute = 60,
    TenMin = 600,
}

impl TimeWindow {
    pub(super) fn variants() -> &'static [TimeWindow] {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_window_next_prev_boundaries() {
        use TimeWindow::*;

        assert_eq!(Second.next(), TenSec);
        assert_eq!(TenSec.next(), Minute);
        assert_eq!(Minute.next(), TenMin);
        assert_eq!(TenMin.next(), TenMin);

        assert_eq!(Second.prev(), Second);
        assert_eq!(TenSec.prev(), Second);
        assert_eq!(Minute.prev(), TenSec);
        assert_eq!(TenMin.prev(), Minute);
    }

    #[test]
    fn time_window_format() {
        assert_eq!(TimeWindow::Second.format(3), "3s");
        assert_eq!(TimeWindow::TenSec.format(3), "30s");
        assert_eq!(TimeWindow::Minute.format(3), "3m");
        assert_eq!(TimeWindow::TenMin.format(3), "30m");
    }
}
