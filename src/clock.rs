//! A pausable logical clock implementation.
//!
//! This module provides [`Clock`] and [`Ticker`] types for measuring elapsed time
//! in benchmark scenarios where the clock may need to be paused (e.g., during warmup).
//!
//! # Overview
//!
//! Unlike a simple `Instant`, the [`Clock`] can be paused and resumed, making it ideal
//! for benchmark frameworks where you want to exclude warmup time or other setup phases
//! from the measured duration.
//!
//! # Example
//!
//! ```ignore
//! use rlt::clock::Clock;
//! use tokio::time::Duration;
//!
//! // Create a paused clock
//! let clock = Clock::new_paused();
//! assert_eq!(clock.elapsed(), Duration::ZERO);
//!
//! // Start the clock
//! clock.resume();
//! tokio::time::sleep(Duration::from_millis(10)).await;
//!
//! // Pause and check elapsed time
//! clock.pause();
//! let elapsed = clock.elapsed();
//! assert!(elapsed >= Duration::from_millis(10));
//! ```

use std::sync::Arc;

use parking_lot::Mutex;
use tokio::time::{self, Duration, Instant};

/// A logical clock that can be paused and resumed.
///
/// This clock tracks elapsed time while accounting for paused periods. When paused,
/// the elapsed time stops accumulating until the clock is resumed.
///
/// The clock is thread-safe and can be cloned to share between multiple tasks.
///
/// # Usage
///
/// ```ignore
/// let clock = Clock::new_paused();
/// clock.resume();  // Start measuring time
/// // ... do work ...
/// clock.pause();   // Stop measuring time
/// let elapsed = clock.elapsed();
/// ```
#[derive(Debug, Clone)]
pub struct Clock {
    #[cfg(feature = "rate_limit")]
    start: Instant,
    inner: Arc<Mutex<InnerClock>>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct InnerClock {
    status: Status,
    elapsed: Duration,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) enum Status {
    #[default]
    Paused,
    Running(Instant),
}

impl Clock {
    fn new(status: Status) -> Self {
        let inner = InnerClock { status, elapsed: Duration::default() };

        cfg_if::cfg_if! {
            if #[cfg(feature = "rate_limit")] {
                Self { start: Instant::now(), inner: Arc::new(Mutex::new(inner)) }
            } else {
                Self { inner: Arc::new(Mutex::new(inner)) }
            }
        }
    }

    /// Creates a new clock that starts running immediately from the given instant.
    ///
    /// # Arguments
    ///
    /// * `start` - The instant to use as the clock's start time.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use rlt::clock::Clock;
    /// use tokio::time::Instant;
    ///
    /// let clock = Clock::start_at(Instant::now());
    /// // Clock is already running, elapsed time will be non-zero after some work
    /// ```
    pub fn start_at(start: Instant) -> Self {
        Self::new(Status::Running(start))
    }

    /// Creates a new clock in paused state.
    /// Call `resume()` to start the clock.
    pub fn new_paused() -> Self {
        Self::new(Status::Paused)
    }

    /// Resumes the clock if it is currently paused.
    ///
    /// If the clock is already running, this method has no effect.
    /// Time will start accumulating from the moment this method is called.
    pub fn resume(&self) {
        let mut inner = self.inner.lock();
        if let Status::Paused = inner.status {
            inner.status = Status::Running(Instant::now());
        }
    }

    /// Pauses the clock if it is currently running.
    ///
    /// If the clock is already paused, this method has no effect.
    /// The elapsed time is preserved and will continue from where it left off
    /// when [`resume()`](Self::resume) is called.
    pub fn pause(&self) {
        let mut inner = self.inner.lock();
        if let Status::Running(checkpoint) = inner.status {
            inner.elapsed += checkpoint.elapsed();
            inner.status = Status::Paused;
        }
    }

    /// Returns the total elapsed time, excluding paused periods.
    ///
    /// If the clock is running, this includes the time since the last resume.
    /// If the clock is paused, this returns the accumulated time up to the pause.
    pub fn elapsed(&self) -> Duration {
        let inner = self.inner.lock();
        match inner.status {
            Status::Paused => inner.elapsed,
            Status::Running(checkpoint) => inner.elapsed + checkpoint.elapsed(),
        }
    }

    /// Sleeps for the specified duration in logical clock time.
    ///
    /// This method accounts for clock pauses. If the clock is paused during the sleep,
    /// the sleep will extend until the logical duration has elapsed.
    ///
    /// # Arguments
    ///
    /// * `duration` - The logical duration to sleep for.
    pub async fn sleep(&self, mut duration: Duration) {
        let wake_time = self.elapsed() + duration;
        loop {
            time::sleep(duration).await;
            let elapsed = self.elapsed();
            if elapsed >= wake_time {
                break;
            }
            duration = wake_time - elapsed;
        }
    }

    /// Sleeps until the specified logical deadline is reached.
    ///
    /// If the deadline has already passed, returns immediately.
    async fn sleep_until(&self, deadline: Duration) {
        let now = self.elapsed();
        if deadline <= now {
            return;
        }
        self.sleep(deadline - now).await;
    }

    /// Creates a [`Ticker`] that ticks at fixed intervals according to this clock.
    ///
    /// The ticker respects clock pauses, so ticks will be delayed while the clock
    /// is paused and will catch up when resumed.
    ///
    /// # Arguments
    ///
    /// * `duration` - The interval between ticks.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let clock = Clock::new_paused();
    /// clock.resume();
    ///
    /// let mut ticker = clock.ticker(Duration::from_millis(100));
    /// ticker.tick().await; // First tick after 100ms
    /// ticker.tick().await; // Second tick after 200ms
    /// ```
    pub fn ticker(&self, duration: Duration) -> Ticker {
        Ticker::new(self.clone(), duration)
    }
}

// the trait `governor::clock::Clock` is not implemented for `&clock::Clock`
#[cfg(feature = "rate_limit")]
impl governor::clock::Clock for Clock {
    type Instant = std::time::Instant;

    fn now(&self) -> Self::Instant {
        let elapsed = self.elapsed();
        self.start.into_std() + elapsed
    }
}
#[cfg(feature = "rate_limit")]
impl governor::clock::ReasonablyRealtime for Clock {}

/// A ticker that produces ticks at fixed intervals according to a logical [`Clock`].
///
/// Unlike [`tokio::time::Interval`], this ticker respects clock pauses:
/// - When the clock is paused, the ticker will wait for the clock to resume.
/// - Ticks are scheduled based on logical elapsed time, not wall-clock time.
///
/// This is useful for implementing rate-limited operations in benchmarks where
/// timing should exclude warmup or other paused periods.
///
/// # Example
///
/// ```ignore
/// let clock = Clock::new_paused();
/// clock.resume();
///
/// let mut ticker = clock.ticker(Duration::from_millis(100));
///
/// // Each tick waits until the next interval
/// ticker.tick().await;  // Fires at 100ms logical time
/// ticker.tick().await;  // Fires at 200ms logical time
/// ```
#[derive(Debug, Clone)]
pub struct Ticker {
    clock: Clock,
    interval: Duration,
    next_tick: Duration,
}

impl Ticker {
    /// Creates a new ticker with the given clock and interval.
    ///
    /// The first tick will occur after `duration` has elapsed on the clock.
    ///
    /// # Arguments
    ///
    /// * `clock` - The clock to use for timing.
    /// * `duration` - The interval between ticks.
    pub fn new(clock: Clock, duration: Duration) -> Self {
        Self { clock, interval: duration, next_tick: duration }
    }

    /// Waits for the next tick.
    ///
    /// This method will block (asynchronously) until the next tick time is reached
    /// according to the clock's logical time. If the clock is paused, this will
    /// wait for the clock to resume before the tick can complete.
    pub async fn tick(&mut self) {
        self.clock.sleep_until(self.next_tick).await;
        self.next_tick += self.interval;
    }
}
