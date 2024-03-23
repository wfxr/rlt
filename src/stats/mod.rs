mod counter;
mod window;

pub use counter::Counter;
pub use window::{RotateDiffWindowGroup, RotateWindowGroup};

use std::collections::HashMap;

use crate::{report::IterReport, status::Status};

#[derive(Clone, Debug)]
pub struct IterStats {
    pub counter: Counter,
    pub details: HashMap<Status, Counter>,
}

impl IterStats {
    pub fn new() -> Self {
        Self { counter: Counter::default(), details: HashMap::new() }
    }
}

impl Default for IterStats {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::AddAssign<&IterReport> for IterStats {
    fn add_assign(&mut self, stats: &IterReport) {
        self.counter += stats;
        let counter = self.details.entry(stats.status).or_default();
        *counter += stats;
    }
}

impl std::ops::Sub<&IterStats> for &IterStats {
    type Output = IterStats;

    fn sub(self, rhs: &IterStats) -> IterStats {
        let mut aggregate = self.counter;
        let mut details = self.details.clone();
        for (k, v) in &rhs.details {
            let counter = details.entry(*k).or_default();
            *counter -= v;
        }
        aggregate -= &rhs.counter;
        IterStats { counter: aggregate, details }
    }
}
