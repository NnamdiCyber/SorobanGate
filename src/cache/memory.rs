use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use moka::sync::Cache as MokaCache;

use super::{Cache, CacheStats};

#[derive(Clone)]
struct CachedValue {
    data: Vec<u8>,
    expires_at: Instant,
}

pub struct MemoryCache {
    cache: MokaCache<u64, CachedValue>,
    hit_count: AtomicU64,
    miss_count: AtomicU64,
    eviction_count: AtomicU64,
}

impl MemoryCache {
    pub fn new(max_memory_mb: u64) -> Self {
        let max_entries = (max_memory_mb * 1024 * 1024) / 1024;
        Self {
            cache: MokaCache::builder().max_capacity(max_entries).build(),
            hit_count: AtomicU64::new(0),
            miss_count: AtomicU64::new(0),
            eviction_count: AtomicU64::new(0),
        }
    }

    fn hash_to_u64(key: &[u8]) -> u64 {
        let mut buf = [0u8; 8];
        let len = key.len().min(8);
        buf[..len].copy_from_slice(&key[..len]);
        u64::from_ne_bytes(buf)
    }
}

impl Cache for MemoryCache {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let k = Self::hash_to_u64(key);
        match self.cache.get(&k) {
            Some(entry) if entry.expires_at > Instant::now() => {
                self.hit_count.fetch_add(1, Ordering::Relaxed);
                Some(entry.data)
            }
            Some(_) => {
                self.miss_count.fetch_add(1, Ordering::Relaxed);
                self.cache.invalidate(&k);
                None
            }
            None => {
                self.miss_count.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    fn set(&self, key: Vec<u8>, value: Vec<u8>, ttl: Duration) {
        let k = Self::hash_to_u64(&key);
        self.cache.insert(
            k,
            CachedValue {
                data: value,
                expires_at: Instant::now() + ttl,
            },
        );
    }

    fn delete(&self, key: &[u8]) -> bool {
        let k = Self::hash_to_u64(key);
        let existed = self.cache.get(&k).is_some();
        self.cache.invalidate(&k);
        existed
    }

    fn flush(&self) {
        self.cache.invalidate_all();
        self.hit_count.store(0, Ordering::Relaxed);
        self.miss_count.store(0, Ordering::Relaxed);
        self.eviction_count.store(0, Ordering::Relaxed);
    }

    fn stats(&self) -> CacheStats {
        CacheStats {
            hit_count: self.hit_count.load(Ordering::Relaxed),
            miss_count: self.miss_count.load(Ordering::Relaxed),
            size: self.cache.entry_count(),
            eviction_count: self.eviction_count.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn test_cache() -> MemoryCache {
        MemoryCache::new(64)
    }

    #[test]
    fn test_cache_hit() {
        let cache = test_cache();
        let key = b"test-key".to_vec();
        let value = b"test-value".to_vec();
        cache.set(key.clone(), value.clone(), Duration::from_secs(60));

        let got = cache.get(&key);
        assert!(got.is_some());
        assert_eq!(got.unwrap(), value);
    }

    #[test]
    fn test_cache_miss() {
        let cache = test_cache();
        let got = cache.get(b"nonexistent");
        assert!(got.is_none());
    }

    #[test]
    fn test_cache_delete() {
        let cache = test_cache();
        let key = b"delete-key".to_vec();
        cache.set(key.clone(), b"value".to_vec(), Duration::from_secs(60));

        assert!(cache.delete(&key));
        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn test_cache_expiry() {
        let cache = test_cache();
        let key = b"expire-key".to_vec();
        cache.set(key.clone(), b"value".to_vec(), Duration::from_millis(10));

        assert!(cache.get(&key).is_some());
        thread::sleep(Duration::from_millis(20));
        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn test_cache_flush() {
        let cache = test_cache();
        cache.set(b"k1".to_vec(), b"v1".to_vec(), Duration::from_secs(60));
        cache.set(b"k2".to_vec(), b"v2".to_vec(), Duration::from_secs(60));

        assert!(cache.get(b"k1").is_some());
        assert!(cache.get(b"k2").is_some());

        cache.flush();

        assert!(cache.get(b"k1").is_none());
        assert!(cache.get(b"k2").is_none());
    }

    #[test]
    fn test_cache_stats() {
        let cache = test_cache();

        let stats = cache.stats();
        assert_eq!(stats.hit_count, 0);
        assert_eq!(stats.miss_count, 0);

        cache.get(b"miss1");
        cache.get(b"miss2");

        let key = b"hit-key".to_vec();
        cache.set(key.clone(), b"val".to_vec(), Duration::from_secs(60));
        cache.get(&key);

        let stats = cache.stats();
        assert_eq!(stats.hit_count, 1);
        assert_eq!(stats.miss_count, 2);
    }

    #[test]
    fn test_cache_different_ttls() {
        let cache = test_cache();
        let k1 = b"short".to_vec();
        let k2 = b"long".to_vec();

        cache.set(k1.clone(), b"v".to_vec(), Duration::from_millis(5));
        cache.set(k2.clone(), b"v".to_vec(), Duration::from_secs(60));

        assert!(cache.get(&k1).is_some());
        assert!(cache.get(&k2).is_some());

        thread::sleep(Duration::from_millis(10));

        assert!(cache.get(&k1).is_none());
        assert!(cache.get(&k2).is_some());
    }

    #[test]
    fn test_hash_to_u64_deterministic() {
        let key1 = b"hello-world";
        let key2 = b"hello-world";
        assert_eq!(
            MemoryCache::hash_to_u64(key1),
            MemoryCache::hash_to_u64(key2)
        );
    }

    #[test]
    fn test_hash_to_u64_different_keys() {
        let key1 = b"key-a";
        let key2 = b"key-b";
        assert_ne!(
            MemoryCache::hash_to_u64(key1),
            MemoryCache::hash_to_u64(key2)
        );
    }
}
