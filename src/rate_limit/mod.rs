pub mod redis;
pub mod token_bucket;

use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct RateLimited;

pub trait RateLimiter: Send + Sync {
    fn check_rate(
        &self,
        key: &str,
        requests_per_second: u32,
        burst: u32,
    ) -> Result<(), RateLimited>;
}

pub enum RateLimiterDispatch {
    Memory(Arc<token_bucket::TokenBucketRateLimiter>),
}

impl RateLimiter for RateLimiterDispatch {
    fn check_rate(
        &self,
        key: &str,
        requests_per_second: u32,
        burst: u32,
    ) -> Result<(), RateLimited> {
        match self {
            RateLimiterDispatch::Memory(limiter) => {
                limiter.check_rate(key, requests_per_second, burst)
            }
        }
    }
}
