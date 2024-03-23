use std::time::Duration;

use crate::report::IterReport;

#[derive(Default, Clone, Copy, Debug)]
pub struct Counter {
    pub iters:    u64,
    pub items:    u64,
    pub bytes:    u64,
    pub duration: Duration,
}

impl std::ops::AddAssign<&IterReport> for Counter {
    fn add_assign(&mut self, stats: &IterReport) {
        self.iters += 1;
        self.items += stats.items;
        self.bytes += stats.bytes;
        self.duration += stats.duration;
    }
}

impl std::ops::SubAssign<&Counter> for Counter {
    fn sub_assign(&mut self, rhs: &Counter) {
        self.iters -= rhs.iters;
        self.items -= rhs.items;
        self.bytes -= rhs.bytes;
        self.duration -= rhs.duration;
    }
}
