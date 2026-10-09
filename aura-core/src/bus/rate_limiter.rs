//! Bus rate limiter and shadow tracking re-exports.
pub use crate::transport::rate_limiter::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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
