//! Duration formatting utilities for human-readable time display.
//!
//! This module provides types and traits for formatting [`Duration`] values
//! with appropriate time units (nanoseconds to hours).
//!
//! # Example
//!
//! ```ignore
//! use std::time::Duration;
//! use rlt::duration::{DurationExt, FormattedDuration, TimeUnit};
//!
//! let d = Duration::from_micros(500);
//! let unit = d.appropriate_unit();  // TimeUnit::Micro
//! println!("{:.2}", FormattedDuration::from(d, unit));  // "500.00µs"
//! ```

use std::time::Duration;

/// A duration paired with a display unit for formatted output.
///
/// This wrapper allows displaying a [`Duration`] in a specific time unit,
/// providing control over the output format.
///
/// # Example
///
/// ```ignore
/// let d = Duration::from_micros(1234);
/// let formatted = FormattedDuration::from(d, TimeUnit::Micro);
/// println!("{:.2}", formatted);  // "1234.00µs"
/// ```
pub struct FormattedDuration {
    duration: Duration,
    unit: TimeUnit,
}

impl FormattedDuration {
    /// Creates a new formatted duration with the specified display unit.
    pub fn from(duration: Duration, unit: TimeUnit) -> Self {
        Self { duration, unit }
    }
}

/// Time units for duration formatting.
///
/// Each variant represents a different time scale, with the discriminant
/// value being the number of nanoseconds in that unit.
#[allow(clippy::enum_clike_unportable_variant)]
#[derive(Debug, Clone, Copy)]
pub enum TimeUnit {
    /// Nanoseconds (ns)
    Nano = 1,
    /// Microseconds (µs)
    Micro = 1_000,
    /// Milliseconds (ms)
    Milli = 1_000_000,
    /// Seconds (s)
    Sec = 1_000_000_000,
    /// Minutes (m)
    Min = 60 * 1_000_000_000,
    /// Hours (h)
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

/// Extension trait for [`Duration`] providing formatting utilities.
pub trait DurationExt {
    /// Returns the most appropriate time unit for displaying this duration.
    ///
    /// Selects the largest unit where the duration is at least 1.0 of that unit,
    /// providing human-readable output without excessive leading zeros or
    /// excessively large numbers.
    fn appropriate_unit(&self) -> TimeUnit;

    /// Converts the duration to a floating-point value in the specified unit.
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
