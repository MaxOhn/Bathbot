use chrono::Utc;
use std::{thread, time::Duration};

/// Basic rate limiter that grants access for a certain amount of times within a time span.
/// Implemented through token bucket algorithm.
pub struct RateLimiter {
    rate: f64,
    per_sec: f64,
    allowance: f64,
    last_call: u64,
    throttle: f64,
}

impl RateLimiter {
    /// Creates a new RateLimiter.
    /// Allows for `rate` amount of access calls within `per_seconds` amount of seconds.
    /// Panics if either argument is less than one.
    pub fn new(rate: u32, per_seconds: u32) -> Self {
        assert!(rate > 0 && per_seconds > 0);
        Self {
            rate: rate as f64,
            per_sec: per_seconds as f64,
            allowance: rate as f64,
            last_call: Utc::now().timestamp_millis() as u64,
            // still not guaranteeing but making it less likely
            // go exceed the desired rate
            throttle: 0.85,
        }
    }

    /// Wait until the next access and take it.
    pub fn await_access(&mut self) {
        let now = Utc::now().timestamp_millis() as u64; // ms
        let elapsed = (now - self.last_call) as f64 / 1000.0; // s
        self.allowance += self.throttle * elapsed * self.rate / self.per_sec; // msgs
        if self.allowance > self.rate {
            self.allowance = self.rate;
        }
        /*
        debug!(
            "Accessing after {}s => allowance: {}",
            elapsed, self.allowance
        );
        */
        if self.allowance < 1.0 {
            let secs_left = (1.0 - self.allowance) * (self.per_sec / self.rate) / self.throttle; // s
            let ms_left = (secs_left * 1000.0).ceil() as u64; // ms
            debug!(" => Sleep {}ms", ms_left);
            thread::sleep(Duration::from_millis(ms_left));
            self.allowance = 0.0;
        } else {
            self.allowance -= 1.0;
        }
        self.last_call = Utc::now().timestamp_millis() as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter() {
        let mut ratelimiter = RateLimiter::new(10, 1);
        let start = Utc::now().timestamp_millis();
        for _ in 0..117 {
            ratelimiter.await_access();
        }
        let end = Utc::now().timestamp_millis();
        let elapsed = end - start;
        println!("RateLimiter elapsed: {}ms", elapsed);
        assert!(elapsed > 11_500);
        assert!(elapsed < 13_500);
    }
}
