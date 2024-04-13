use std::sync::Arc;

use parking_lot::Mutex;
use tokio::time::{self, Duration, Instant};

/// A logical clock that can be paused
#[derive(Debug, Clone, Default)]
pub struct Clock {
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
    pub fn start_at(start_time: Instant) -> Self {
        let inner = InnerClock {
            status: Status::Running(start_time),
            elapsed: Duration::default(),
        };
        Self { inner: Arc::new(Mutex::new(inner)) }
    }

    pub fn run(&mut self) {
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

/// A ticker that ticks at a fixed logical interval
#[derive(Debug, Clone)]
pub struct Ticker {
    clock: Clock,
    duration: Duration,
    next: Duration,
}

impl Ticker {
    pub fn new(clock: Clock, duration: Duration) -> Self {
        Self { clock, duration, next: duration }
    }

    pub async fn tick(&mut self) {
        self.clock.sleep_until(self.next).await;
        self.next += self.duration;
    }
}
