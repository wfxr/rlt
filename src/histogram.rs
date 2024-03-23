use std::time::Duration;

use hdrhistogram::{Histogram, RecordError};

pub const PERCENTAGES: &[f64] = &[10.0, 25.0, 50.0, 75.0, 90.0, 95.0, 99.0, 99.9, 99.99];

pub struct LatencyHistogram {
    hist: Histogram<u64>,
}

impl LatencyHistogram {
    pub fn new() -> LatencyHistogram {
        Self { hist: Histogram::<u64>::new(3).expect("create histogram") }
    }

    pub fn record(&mut self, d: Duration) -> Result<(), RecordError> {
        self.hist.record(d.as_nanos() as u64)
    }

    pub fn is_empty(&self) -> bool {
        self.hist.is_empty()
    }

    pub fn max(&self) -> Duration {
        Duration::from_nanos(self.hist.max())
    }

    pub fn min(&self) -> Duration {
        Duration::from_nanos(self.hist.min())
    }

    pub fn mean(&self) -> Duration {
        Duration::from_nanos(self.hist.mean() as u64)
    }

    pub fn stdev(&self) -> Duration {
        Duration::from_nanos(self.hist.stdev() as u64)
    }

    pub fn median(&self) -> Duration {
        self.value_at_quantile(0.5)
    }

    pub fn value_at_quantile(&self, q: f64) -> Duration {
        Duration::from_nanos(self.hist.value_at_quantile(q))
    }

    pub fn quantiles(&self) -> impl Iterator<Item = (Duration, u64)> + '_ {
        self.hist
            .iter_quantiles(1)
            .map(|t| {
                (
                    Duration::from_nanos(t.value_iterated_to()),
                    t.count_since_last_iteration(),
                )
            })
            .filter(|(_, n)| *n > 0)
    }

    pub fn percentiles<'a>(&'a self, percentages: &'a [f64]) -> impl Iterator<Item = (f64, Duration)> + 'a {
        percentages.iter().map(|&p| (p, self.value_at_quantile(p / 100.0)))
    }
}

impl Default for LatencyHistogram {
    fn default() -> Self {
        Self::new()
    }
}
