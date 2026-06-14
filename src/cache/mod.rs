pub mod memory;

use std::time::Duration;
use std::collections::HashMap;
use sha2::{Sha256, Digest};

#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    pub hit_count: u64,
    pub miss_count: u64,
    pub size: u64,
    pub eviction_count: u64,
}

pub trait Cache: Send + Sync {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    fn set(&self, key: Vec<u8>, value: Vec<u8>, ttl: Duration);
    fn delete(&self, key: &[u8]) -> bool;
    fn flush(&self);
    fn stats(&self) -> CacheStats;
}

#[derive(Clone)]
pub struct TtlTable {
    rules: HashMap<String, u64>,
}

impl TtlTable {
    pub fn new(rules: &[crate::config::CacheRule]) -> Self {
        let mut map = HashMap::new();
        for rule in rules {
            for method in &rule.methods {
                map.insert(method.clone(), rule.ttl_secs);
            }
        }
        Self { rules: map }
    }

    pub fn ttl_for(&self, method: &str) -> Duration {
        match self.rules.get(method) {
            Some(0) | None => Duration::ZERO,
            Some(secs) => Duration::from_secs(*secs),
        }
    }
}

pub fn compute_cache_key(body: &[u8]) -> Vec<u8> {
    let mut value: serde_json::Value = serde_json::from_slice(body)
        .unwrap_or(serde_json::Value::Null);
    if let serde_json::Value::Object(ref mut obj) = value {
        obj.remove("id");
        obj.remove("jsonrpc");
    }
    let canonical = serde_json::to_vec(&value).unwrap_or_else(|_| body.to_vec());

    let mut hasher = Sha256::new();
    hasher.update(&canonical);
    hasher.finalize().to_vec()
}
