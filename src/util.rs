//! Utility functions and traits for byte formatting and rate calculations.
//!
//! This module provides helpers for:
//! - Converting byte counts to human-readable formats (KiB, MiB, GiB, etc.)
//! - Calculating rates safely (handling division by zero)

use byte_unit::{Byte, UnitType};

fn format_byte(byte: Byte, precision: usize) -> String {
    format!(
        "{:.prec$}",
        byte.get_appropriate_unit(UnitType::Binary),
        prec = precision
    )
}

/// Trait for formatting a value as a human-readable byte string.
pub trait HumanBytes {
    /// Formats the value as bytes with the given precision.
    ///
    /// Returns "N/A" for values that cannot be represented (e.g., NaN, Inf, negative).
    fn human_bytes(self, precision: usize) -> String;
}

impl HumanBytes for f64 {
    fn human_bytes(self, precision: usize) -> String {
        Byte::from_f64(self)
            .map(|b| format_byte(b, precision))
            .unwrap_or_else(|| "N/A".to_string())
    }
}

impl HumanBytes for u64 {
    fn human_bytes(self, precision: usize) -> String {
        format_byte(Byte::from_u64(self), precision)
    }
}

/// Calculates rate safely, returning 0.0 if elapsed time is zero or negative.
///
/// # Arguments
///
/// * `count` - The total count (iterations, bytes, items, etc.)
/// * `elapsed` - The elapsed time in seconds
///
/// # Returns
///
/// The rate as count per second, or 0.0 if elapsed is not positive.
pub fn rate(count: u64, elapsed: f64) -> f64 {
    if elapsed > 0.0 { count as f64 / elapsed } else { 0.0 }
}
