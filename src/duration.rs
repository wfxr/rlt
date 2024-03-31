use std::time::Duration;

pub struct FormattedDuration {
    duration: Duration,
    unit: TimeUnit,
}

impl FormattedDuration {
    pub fn from(duration: Duration, unit: TimeUnit) -> Self {
        Self { duration, unit }
    }
}

#[allow(clippy::enum_clike_unportable_variant)]
#[derive(Debug, Clone, Copy)]
pub enum TimeUnit {
    Nano = 1,
    Micro = 1_000,
    Milli = 1_000_000,
    Sec = 1_000_000_000,
    Min = 60 * 1_000_000_000,
    Hour = 60 * 60 * 1_000_000_000,
}

impl std::fmt::Display for TimeUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let unit = match self {
            TimeUnit::Nano => "ns",
            TimeUnit::Micro => "µs",
            TimeUnit::Milli => "ms",
            TimeUnit::Sec => "s",
            TimeUnit::Min => "m",
            TimeUnit::Hour => "h",
        };
        write!(f, "{}", unit)
    }
}

impl std::fmt::Display for FormattedDuration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.duration
            .as_f64(self.unit)
            .fmt(f)
            .and_then(|_| write!(f, "{unit}", unit = self.unit,))
    }
}

pub trait DurationExt {
    fn appropriate_unit(&self) -> TimeUnit;
    fn as_f64(&self, unit: TimeUnit) -> f64;
}

impl DurationExt for Duration {
    fn appropriate_unit(&self) -> TimeUnit {
        let duration = *self;
        match duration.as_nanos() {
            n if n < TimeUnit::Micro as u128 => TimeUnit::Nano,
            n if n < TimeUnit::Milli as u128 => TimeUnit::Micro,
            n if n < TimeUnit::Sec as u128 => TimeUnit::Milli,
            n if n < TimeUnit::Min as u128 => TimeUnit::Sec,
            n if n < TimeUnit::Hour as u128 => TimeUnit::Min,
            _ => TimeUnit::Hour,
        }
    }

    fn as_f64(&self, unit: TimeUnit) -> f64 {
        self.as_nanos() as f64 / unit as u64 as f64
    }
}
