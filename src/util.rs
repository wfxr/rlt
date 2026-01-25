//! Utility functions and traits for byte formatting and rate calculations.
//!
//! This module provides helpers for:
//! - Converting byte counts to human-readable formats (KiB, MiB, GiB, etc.)
//! - Calculating rates safely (handling division by zero)

use anyhow::anyhow;
use byte_unit::{Byte, UnitType};

/// Trait for converting a value to a human-readable byte representation.
///
/// This fallible version is used for floating-point values that might
/// be too large to represent.
pub trait TryIntoAdjustedByte {
    /// Converts the value to an appropriate byte unit (KiB, MiB, GiB, etc.).
    ///
    /// # Errors
    ///
    /// Returns an error if the value is too large to represent as bytes.
    fn adjusted(self) -> anyhow::Result<byte_unit::AdjustedByte>;
}

/// Trait for converting a value to a human-readable byte representation.
///
/// This infallible version is used for integer values that are always representable.
pub trait IntoAdjustedByte {
    /// Converts the value to an appropriate byte unit (KiB, MiB, GiB, etc.).
    fn adjusted(self) -> byte_unit::AdjustedByte;
}

impl TryIntoAdjustedByte for f64 {
    fn adjusted(self) -> anyhow::Result<byte_unit::AdjustedByte> {
        Byte::from_f64(self)
            .ok_or(anyhow!("size too large"))
            .map(|b| b.get_appropriate_unit(UnitType::Binary))
    }
}

impl IntoAdjustedByte for u64 {
    fn adjusted(self) -> byte_unit::AdjustedByte {
        Byte::from_u64(self).get_appropriate_unit(UnitType::Binary)
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
