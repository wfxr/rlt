//! A simple wrapper around [`hdrhistogram::Histogram`] for latency measurements.
use std::time::Duration;

use hdrhistogram::Histogram;

use crate::error::CollectorError;

pub(crate) const PERCENTAGES: &[f64] = &[10.0, 25.0, 50.0, 75.0, 90.0, 95.0, 99.0, 99.9, 99.99];

/// A simple wrapper around [`hdrhistogram::Histogram`] for latency measurements.
pub struct LatencyHistogram {
    hist: Histogram<u64>,
}

impl LatencyHistogram {
    /// Creates a new latency histogram.
    pub fn new() -> LatencyHistogram {
        Self { hist: Histogram::<u64>::new(3).expect("create histogram") }
    }

    /// Records a latency value.
    pub fn record(&mut self, d: Duration) -> std::result::Result<(), CollectorError> {
        let nanos = u64::try_from(d.as_nanos())
            .map_err(|_| CollectorError::LatencyTooLarge { latency: d })?;
        self.hist.record(nanos).map_err(CollectorError::HistogramRecord)
    }

    /// Returns true if this histogram has no recorded values.
    pub fn is_empty(&self) -> bool {
        self.hist.is_empty()
    }

    /// Get the highest recorded latency in the histogram.
    pub fn max(&self) -> Duration {
        Duration::from_nanos(self.hist.max())
    }

    /// Get the lowest recorded latency in the histogram.
    pub fn min(&self) -> Duration {
        Duration::from_nanos(self.hist.min())
    }

    /// Get the computed mean value of all recorded latencies in the histogram.
    pub fn mean(&self) -> Duration {
        Duration::from_nanos(self.hist.mean() as u64)
    }

    /// Get the computed standard deviation of all recorded latencies in the histogram.
    pub fn stdev(&self) -> Duration {
        Duration::from_nanos(self.hist.stdev() as u64)
    }

    /// Get the computed median value of all recorded latencies in the histogram.
    pub fn median(&self) -> Duration {
        self.value_at_quantile(0.5)
    }

    /// Get the latency at a given quantile.
    pub fn value_at_quantile(&self, q: f64) -> Duration {
        Duration::from_nanos(self.hist.value_at_quantile(q))
    }

    /// Iterate through histogram values by quantile levels.
    ///
    /// See [`hdrhistogram::Histogram::iter_quantiles`] for more details.
    pub fn quantiles(&self) -> impl Iterator<Item = (Duration, u64)> + '_ {
        self.hist
            .iter_quantiles(1)
            .map(|t| (Duration::from_nanos(t.value_iterated_to()), t.count_since_last_iteration()))
            .filter(|(_, n)| *n > 0)
    }

    /// Compute each latency value at the given percentages.
    pub fn percentiles<'a>(
        &'a self,
        percentages: &'a [f64],
    ) -> impl Iterator<Item = (f64, Duration)> + 'a {
        percentages.iter().map(|&p| (p, self.value_at_quantile(p / 100.0)))
    }
}

impl Default for LatencyHistogram {
    fn default() -> Self {
        Self::new()
    }
}
