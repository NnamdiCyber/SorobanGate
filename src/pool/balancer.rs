use rand::Rng;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::UpstreamState;

/// Trait for load balancing algorithms.
pub trait LoadBalancer: Send + Sync {
    /// Select an upstream from the list of healthy upstreams.
    fn select(&self, healthy: &[Arc<UpstreamState>]) -> Option<Arc<UpstreamState>>;
}

/// Weighted round-robin load balancer.
///
/// Distributes requests proportionally to each upstream's weight using
/// an atomic counter for thread-safe round-robin iteration.
pub struct WeightedRoundRobin {
    counter: AtomicU64,
}

impl WeightedRoundRobin {
    pub fn new() -> Self {
        Self {
            counter: AtomicU64::new(0),
        }
    }
}

impl Default for WeightedRoundRobin {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadBalancer for WeightedRoundRobin {
    fn select(&self, healthy: &[Arc<UpstreamState>]) -> Option<Arc<UpstreamState>> {
        weighted_round_robin(healthy, &self.counter)
    }
}

/// Least-connections load balancer.
///
/// Picks the upstream with the fewest active connections.
pub struct LeastConnections;

impl LoadBalancer for LeastConnections {
    fn select(&self, healthy: &[Arc<UpstreamState>]) -> Option<Arc<UpstreamState>> {
        least_connections(healthy)
    }
}

/// Random load balancer.
///
/// Picks a random upstream from the healthy set.
pub struct Random;

impl LoadBalancer for Random {
    fn select(&self, healthy: &[Arc<UpstreamState>]) -> Option<Arc<UpstreamState>> {
        random(healthy)
    }
}

/// Weighted round-robin selection from healthy upstreams.
///
/// Builds a virtual list where each upstream appears `weight` times,
/// then selects using modulo of an atomic counter.
pub fn weighted_round_robin(
    healthy: &[Arc<UpstreamState>],
    counter: &AtomicU64,
) -> Option<Arc<UpstreamState>> {
    if healthy.is_empty() {
        return None;
    }

    let mut weighted: Vec<usize> = Vec::with_capacity(healthy.len());
    for (i, u) in healthy.iter().enumerate() {
        let w = u.weight.max(1) as usize;
        for _ in 0..w {
            weighted.push(i);
        }
    }

    if weighted.is_empty() {
        return None;
    }

    let count = counter.fetch_add(1, Ordering::Relaxed);
    let idx = weighted[count as usize % weighted.len()];
    Some(healthy[idx].clone())
}

/// Least-connections selection from healthy upstreams.
pub fn least_connections(healthy: &[Arc<UpstreamState>]) -> Option<Arc<UpstreamState>> {
    healthy
        .iter()
        .min_by_key(|u| u.active_connections())
        .cloned()
}

/// Random selection from healthy upstreams.
pub fn random(healthy: &[Arc<UpstreamState>]) -> Option<Arc<UpstreamState>> {
    if healthy.is_empty() {
        return None;
    }
    let idx = rand::thread_rng().gen_range(0..healthy.len());
    Some(healthy[idx].clone())
}

/// Select an upstream using the configured algorithm.
pub fn select(
    healthy: &[Arc<UpstreamState>],
    algorithm: &crate::config::LoadBalancingAlgorithm,
    wrr_counter: &AtomicU64,
) -> Option<Arc<UpstreamState>> {
    match algorithm {
        crate::config::LoadBalancingAlgorithm::Wrr => {
            weighted_round_robin(healthy, wrr_counter)
        }
        crate::config::LoadBalancingAlgorithm::Lc => least_connections(healthy),
        crate::config::LoadBalancingAlgorithm::Random => random(healthy),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::circuit_breaker::CircuitBreaker;
    use crate::pool::{HealthStatus, UpstreamMutableState};
    use std::sync::atomic::AtomicI64;
    use std::sync::atomic::AtomicU64;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    fn upstream(url: &str, weight: u32, healthy: bool, connections: i64) -> Arc<UpstreamState> {
        let mut cb = CircuitBreaker::new(5, Duration::from_secs(30));
        if !healthy {
            // Mark circuit breaker as open so is_available() returns false
            for _ in 0..5 {
                cb.record_failure();
            }
        }
        let state = UpstreamState {
            pool_name: "test".to_string(),
            url: url.to_string(),
            weight,
            max_connections: None,
            mutable: Mutex::new(UpstreamMutableState {
                health: if healthy {
                    HealthStatus::Healthy
                } else {
                    HealthStatus::Unhealthy
                },
                circuit_breaker: cb,
                consecutive_successes: 0,
                consecutive_failures: 0,
            }),
            connection_count: AtomicI64::new(connections),
            latency_ns: AtomicU64::new(0),
        };
        Arc::new(state)
    }

    fn healthy(url: &str, weight: u32) -> Arc<UpstreamState> {
        upstream(url, weight, true, 0)
    }

    // ── WRR tests ──

    #[test]
    fn test_wrr_empty_returns_none() {
        let counter = AtomicU64::new(0);
        assert!(weighted_round_robin(&[], &counter).is_none());
    }

    #[test]
    fn test_wrr_single_upstream_always_returns_it() {
        let u = healthy("a", 1);
        let counter = AtomicU64::new(0);
        for _ in 0..10 {
            let selected = weighted_round_robin(std::slice::from_ref(&u), &counter).unwrap();
            assert_eq!(selected.url, "a");
        }
    }

    #[test]
    fn test_wrr_distribution_proportional_to_weights() {
        let u1 = healthy("heavy", 3);
        let u2 = healthy("light", 1);
        let healthy = vec![u1, u2];
        let counter = AtomicU64::new(0);

        let mut counts = std::collections::HashMap::new();
        for _ in 0..400 {
            let selected = weighted_round_robin(&healthy, &counter).unwrap();
            *counts.entry(selected.url.clone()).or_insert(0) += 1;
        }

        // With weights 3:1, expect ~300 heavy and ~100 light
        let heavy = *counts.get("heavy").unwrap();
        let light = *counts.get("light").unwrap();
        assert!(
            heavy > 200,
            "heavy should be >200, got {heavy}"
        );
        assert!(
            light > 50,
            "light should be >50, got {light}"
        );
        // Ratio check: heavy ≈ 3 × light
        let ratio = heavy as f64 / light as f64;
        assert!(
            (2.0..=4.0).contains(&ratio),
            "heavy/light ratio should be ~3, got {ratio}"
        );
    }

    #[test]
    fn test_wrr_cycles_correctly() {
        let u1 = healthy("a", 1);
        let u2 = healthy("b", 2);
        let healthy = vec![u1, u2];
        let counter = AtomicU64::new(0);

        // Weighted list: [a, b, b] → indices [0, 1, 1]
        // Cycle should be: a, b, b, a, b, b, ...
        let expected_order = ["a", "b", "b", "a", "b", "b"];
        for expected in &expected_order {
            let selected = weighted_round_robin(&healthy, &counter).unwrap();
            assert_eq!(selected.url, *expected);
        }
    }

    #[test]
    fn test_wrr_zero_weight_treated_as_one() {
        let u1 = healthy("a", 0);
        let u2 = healthy("b", 1);
        let healthy = vec![u1, u2];
        let counter = AtomicU64::new(0);

        // Both effectively weight 1, cycle: a, b, a, b, ...
        assert_eq!(
            weighted_round_robin(&healthy, &counter).unwrap().url,
            "a"
        );
        assert_eq!(
            weighted_round_robin(&healthy, &counter).unwrap().url,
            "b"
        );
    }

    // ── LC tests ──

    #[test]
    fn test_lc_empty_returns_none() {
        assert!(least_connections(&[]).is_none());
    }

    #[test]
    fn test_lc_picks_upstream_with_fewest_connections() {
        let u1 = upstream("busy", 1, true, 10);
        let u2 = upstream("idle", 1, true, 2);
        let u3 = upstream("medium", 1, true, 5);
        let healthy = vec![u1, u2, u3];

        let selected = least_connections(&healthy).unwrap();
        assert_eq!(selected.url, "idle");
    }

    #[test]
    fn test_lc_returns_single_upstream() {
        let u = healthy("only", 1);
        let selected = least_connections(std::slice::from_ref(&u)).unwrap();
        assert_eq!(selected.url, "only");
    }

    #[test]
    fn test_lc_picks_first_when_tied() {
        let u1 = upstream("a", 1, true, 5);
        let u2 = upstream("b", 1, true, 5);
        let healthy = vec![u1, u2];

        let selected = least_connections(&healthy).unwrap();
        // When tied, min_by_key returns the first
        assert_eq!(selected.url, "a");
    }

    // ── Random tests ──

    #[test]
    fn test_random_empty_returns_none() {
        assert!(random(&[]).is_none());
    }

    #[test]
    fn test_random_never_returns_unhealthy() {
        let healthy_upstreams = vec![healthy("good", 1), healthy("good2", 1)];

        for _ in 0..100 {
            let selected = random(&healthy_upstreams).unwrap();
            assert!(selected.is_available());
        }
    }

    #[test]
    fn test_random_returns_from_available_set() {
        let urls = ["x", "y", "z"];
        let upstreams: Vec<_> = urls.iter().map(|u| healthy(u, 1)).collect();

        for _ in 0..100 {
            let selected = random(&upstreams).unwrap();
            assert!(urls.contains(&selected.url.as_str()));
        }
    }

    // ── Trait tests (polymorphic dispatch) ──

    #[test]
    fn test_trait_dispatch_wrr() {
        let u = healthy("a", 1);
        let balancer = WeightedRoundRobin::new();
        let selected = balancer.select(std::slice::from_ref(&u)).unwrap();
        assert_eq!(selected.url, "a");
    }

    #[test]
    fn test_trait_dispatch_lc() {
        let u1 = upstream("busy", 1, true, 10);
        let u2 = upstream("idle", 1, true, 1);
        let balancer = LeastConnections;
        let selected = balancer.select(&[u1, u2]).unwrap();
        assert_eq!(selected.url, "idle");
    }

    #[test]
    fn test_trait_dispatch_random() {
        let u = healthy("a", 1);
        let balancer = Random;
        let selected = balancer.select(std::slice::from_ref(&u)).unwrap();
        assert_eq!(selected.url, "a");
    }

    // ── Convenience select function ──

    #[test]
    fn test_select_dispatches_wrr() {
        let u = healthy("a", 1);
        let counter = AtomicU64::new(0);
        let selected = select(
            std::slice::from_ref(&u),
            &crate::config::LoadBalancingAlgorithm::Wrr,
            &counter,
        )
        .unwrap();
        assert_eq!(selected.url, "a");
    }

    #[test]
    fn test_select_dispatches_lc() {
        let u = healthy("a", 1);
        let selected = select(
            std::slice::from_ref(&u),
            &crate::config::LoadBalancingAlgorithm::Lc,
            &AtomicU64::new(0),
        )
        .unwrap();
        assert_eq!(selected.url, "a");
    }

    #[test]
    fn test_select_dispatches_random() {
        let u = healthy("a", 1);
        let selected = select(
            std::slice::from_ref(&u),
            &crate::config::LoadBalancingAlgorithm::Random,
            &AtomicU64::new(0),
        )
        .unwrap();
        assert_eq!(selected.url, "a");
    }

    #[test]
    fn test_select_empty_returns_none() {
        let result = select(
            &[],
            &crate::config::LoadBalancingAlgorithm::Wrr,
            &AtomicU64::new(0),
        );
        assert!(result.is_none());
    }
}
