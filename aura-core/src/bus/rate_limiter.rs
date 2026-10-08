use dashmap::DashMap;
use std::time::{Duration, Instant};

struct TokenBucket {
    tokens: u32,
    last_refill: Instant,
}

/// Token Bucket rate limiter for managing event bus backpressure (BUS-008, ADR-001).
pub struct RateLimiter {
    buckets: DashMap<String, TokenBucket>,
    capacity: u32,
    refill_rate: Duration,
    refill_amount: u32,
}

impl RateLimiter {
    pub fn new(capacity: u32, refill_amount: u32, refill_rate: Duration) -> Self {
        Self {
            buckets: DashMap::new(),
            capacity,
            refill_amount,
            refill_rate,
        }
    }

    /// Checks if a client with the given SPIFFE ID is allowed to consume a token.
    /// Returns true if allowed, or false if rate limit exceeded.
    pub fn check_and_consume(&self, spiffe_id: &str) -> bool {
        let mut bucket = self
            .buckets
            .entry(spiffe_id.to_string())
            .or_insert_with(|| TokenBucket {
                tokens: self.capacity,
                last_refill: Instant::now(),
            });

        let now = Instant::now();
        let elapsed = now.duration_since(bucket.last_refill);

        if elapsed >= self.refill_rate {
            let refills = (elapsed.as_millis() / self.refill_rate.as_millis()) as u32;
            let new_tokens = bucket.tokens + refills * self.refill_amount;
            bucket.tokens = new_tokens.min(self.capacity);
            bucket.last_refill = now;
        }

        if bucket.tokens > 0 {
            bucket.tokens -= 1;
            true
        } else {
            false
        }
    }

    /// Resets the rate limit bucket for a specific SPIFFE ID.
    pub fn reset(&self, spiffe_id: &str) {
        self.buckets.remove(spiffe_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter() {
        let limiter = RateLimiter::new(2, 1, Duration::from_millis(100));
        assert!(limiter.check_and_consume("spiffe://test"));
        assert!(limiter.check_and_consume("spiffe://test"));
        assert!(!limiter.check_and_consume("spiffe://test"));
        std::thread::sleep(Duration::from_millis(110));
        assert!(limiter.check_and_consume("spiffe://test"));
        assert!(!limiter.check_and_consume("spiffe://test"));
    }
}
