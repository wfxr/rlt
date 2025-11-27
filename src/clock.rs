use std::sync::Arc;

use parking_lot::Mutex;
use tokio::time::{self, Duration, Instant};

/// A logical clock that can be paused
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
    /// Create a new clock that starts running immediately.
    pub fn start_at(start: Instant) -> Self {
        let inner = InnerClock { status: Status::Running(start), elapsed: Duration::default() };

        cfg_if::cfg_if! {
            if #[cfg(feature = "rate_limit")] {
                Self { start, inner: Arc::new(Mutex::new(inner)) }
            } else {
                Self { inner: Arc::new(Mutex::new(inner)) }
            }
        }
    }

    /// Create a new clock in paused state.
    /// Call `resume()` to start the clock.
    pub fn new_paused() -> Self {
        let inner = InnerClock { status: Status::Paused, elapsed: Duration::default() };

        cfg_if::cfg_if! {
            if #[cfg(feature = "rate_limit")] {
                Self { start: Instant::now(), inner: Arc::new(Mutex::new(inner)) }
            } else {
                Self { inner: Arc::new(Mutex::new(inner)) }
            }
        }
    }

    pub fn resume(&self) {
        let mut inner = self.inner.lock();
        if let Status::Paused = inner.status {
            inner.status = Status::Running(Instant::now());
        }
    }

    pub fn pause(&self) {
        let mut inner = self.inner.lock();
        if let Status::Running(checkpoint) = inner.status {
            inner.elapsed += checkpoint.elapsed();
            inner.status = Status::Paused;
        }
    }

    pub fn elapsed(&self) -> Duration {
        let inner = self.inner.lock();
        match inner.status {
            Status::Paused => inner.elapsed,
            Status::Running(checkpoint) => inner.elapsed + checkpoint.elapsed(),
        }
    }

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

    async fn sleep_until(&self, deadline: Duration) {
        let now = self.elapsed();
        if deadline <= now {
            return;
        }
        self.sleep(deadline - now).await;
    }

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

/// A ticker that ticks at a fixed logical interval
#[derive(Debug, Clone)]
pub struct Ticker {
    clock: Clock,
    interval: Duration,
    next_tick: Duration,
}

impl Ticker {
    pub fn new(clock: Clock, duration: Duration) -> Self {
        Self { clock, interval: duration, next_tick: duration }
    }

    pub async fn tick(&mut self) {
        self.clock.sleep_until(self.next_tick).await;
        self.next_tick += self.interval;
    }
}
