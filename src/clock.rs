use std::sync::Arc;

use parking_lot::Mutex;
use tokio::time::{self, Duration, Instant};

/// A logical clock that can be paused
#[derive(Debug, Clone)]
pub struct Clock {
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
    pub fn start_at(start: Instant) -> Self {
        let inner = InnerClock { status: Status::Running(start), elapsed: Duration::default() };
        Self { start, inner: Arc::new(Mutex::new(inner)) }
    }

    pub fn resume(&mut self) {
        let mut inner = self.inner.lock();
        if let Status::Paused = inner.status {
            inner.status = Status::Running(Instant::now());
        }
    }

    pub fn pause(&mut self) {
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

impl governor::clock::Clock for Clock {
    type Instant = std::time::Instant;

    fn now(&self) -> Self::Instant {
        let elapsed = self.elapsed();
        self.start.into_std() + elapsed
    }
}

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
