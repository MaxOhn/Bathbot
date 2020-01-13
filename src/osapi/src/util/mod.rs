use chrono::Utc;
use std::{collections::VecDeque, thread, time::Duration};

/// Very basic rate limiter that grants access for a certain amount of times within a time span.
pub(crate) struct RateLimiter {
    time_span: u64,
    limit: usize,
    entries: VecDeque<u64>,
}

impl RateLimiter {
    /// Creates a new RateLimiter.
    /// Allows for `limit` amount of access calls within `time_span_ms` amount of milliseconds.
    /// Panics if either argument is less than one.
    pub(crate) fn new(time_span_ms: u64, limit: usize) -> Self {
        assert!(time_span_ms > 0 && limit > 0);
        RateLimiter {
            time_span: time_span_ms,
            limit,
            entries: VecDeque::with_capacity(limit),
        }
    }

    /// Check whether current access is possible.
    /// If so, take it.
    pub(crate) fn try_access(&mut self) -> bool {
        let time = Utc::now().timestamp_millis() as u64;
        self.update(time);
        if self.entries.len() == self.limit {
            return false;
        }
        self.entries.push_back(time);
        true
    }

    /// Wait until the next access and take it.
    pub(crate) fn wait_access(&mut self) {
        let time = Utc::now().timestamp_millis() as u64;
        self.update(time);
        if self.entries.len() == self.limit {
            let next = self.entries.pop_front().unwrap();
            thread::sleep(Duration::from_millis(time - next));
        }
        self.entries.push_back(time);
    }

    /// Private function to remove all entries that happened sufficiently long ago
    fn update(&mut self, time: u64) {
        let start = time - self.time_span;
        while let Some(front) = self.entries.front() {
            if *front >= start {
                break;
            }
            self.entries.pop_front();
        }
    }
}
