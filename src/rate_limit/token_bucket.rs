use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

pub struct TokenBucketRateLimiter {
    buckets: Mutex<HashMap<String, Bucket>>,
}

impl TokenBucketRateLimiter {
    pub fn new() -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for TokenBucketRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenBucketRateLimiter {
    pub fn check_rate(
        &self,
        key: &str,
        requests_per_second: u32,
        burst: u32,
    ) -> Result<(), super::RateLimited> {
        let mut buckets = self.buckets.lock().unwrap();
        let now = Instant::now();
        let bucket = buckets.entry(key.to_string()).or_insert(Bucket {
            tokens: burst as f64,
            last_refill: now,
        });

        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * requests_per_second as f64).min(burst as f64);
        bucket.last_refill = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Ok(())
        } else {
            Err(super::RateLimited)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_burst_allowed() {
        let limiter = TokenBucketRateLimiter::new();
        for i in 0..10 {
            assert!(
                limiter.check_rate("test_burst_allowed", 5, 10).is_ok(),
                "request {} should be allowed",
                i
            );
        }
        assert!(limiter.check_rate("test_burst_allowed", 5, 10).is_err());
    }

    #[test]
    fn test_different_keys_independent() {
        let limiter = TokenBucketRateLimiter::new();
        for _ in 0..10 {
            let _ = limiter.check_rate("key_a", 5, 10);
        }
        assert!(limiter.check_rate("key_a", 5, 10).is_err());
        assert!(limiter.check_rate("key_b", 5, 10).is_ok());
    }

    #[test]
    fn test_refill_over_time() {
        let limiter = TokenBucketRateLimiter::new();
        assert!(limiter.check_rate("test_refill", 10, 1).is_ok());
        assert!(limiter.check_rate("test_refill", 10, 1).is_err());
        thread::sleep(Duration::from_millis(200));
        assert!(limiter.check_rate("test_refill", 10, 1).is_ok());
    }

    #[test]
    fn test_sustained_over_rate_rejected() {
        let limiter = TokenBucketRateLimiter::new();
        assert!(limiter.check_rate("test_sustained", 1, 1).is_ok());
        assert!(limiter.check_rate("test_sustained", 1, 1).is_err());
    }

    #[test]
    fn test_ip_fallback_uses_separate_bucket() {
        let limiter = TokenBucketRateLimiter::new();
        assert!(limiter.check_rate("ip:192.168.1.1", 5, 10).is_ok());
        for _ in 0..9 {
            let _ = limiter.check_rate("ip:192.168.1.1", 5, 10);
        }
        assert!(limiter.check_rate("ip:192.168.1.1", 5, 10).is_err());
        assert!(limiter.check_rate("ip:10.0.0.1", 5, 10).is_ok());
    }
}
