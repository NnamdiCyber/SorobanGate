use std::sync::atomic::AtomicI64;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

pub mod balancer;
pub mod circuit_breaker;
pub mod health;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
}

pub struct UpstreamMutableState {
    pub health: HealthStatus,
    pub circuit_breaker: circuit_breaker::CircuitBreaker,
    pub consecutive_successes: u32,
    pub consecutive_failures: u32,
}

pub struct UpstreamState {
    pub pool_name: String,
    pub url: String,
    pub weight: u32,
    pub max_connections: Option<u32>,
    pub mutable: Mutex<UpstreamMutableState>,
    pub connection_count: AtomicI64,
    pub latency_ns: AtomicU64,
}

impl UpstreamState {
    pub fn new(pool_name: String, url: String, weight: u32, max_connections: Option<u32>) -> Self {
        Self {
            pool_name,
            url,
            weight,
            max_connections,
            mutable: Mutex::new(UpstreamMutableState {
                health: HealthStatus::Unhealthy,
                circuit_breaker: circuit_breaker::CircuitBreaker::new(
                    5,
                    std::time::Duration::from_secs(30),
                ),
                consecutive_successes: 0,
                consecutive_failures: 0,
            }),
            connection_count: AtomicI64::new(0),
            latency_ns: AtomicU64::new(0),
        }
    }

    pub fn is_available(&self) -> bool {
        let state = self.mutable.lock().unwrap();
        state.health == HealthStatus::Healthy && state.circuit_breaker.is_available()
    }

    pub fn health_status(&self) -> HealthStatus {
        self.mutable.lock().unwrap().health
    }

    pub fn circuit_breaker_state(&self) -> circuit_breaker::CircuitBreakerState {
        self.mutable.lock().unwrap().circuit_breaker.state()
    }

    pub fn increment_connection_count(&self) {
        self.connection_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn decrement_connection_count(&self) {
        self.connection_count.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn active_connections(&self) -> i64 {
        self.connection_count.load(Ordering::Relaxed)
    }

    pub fn latency_us(&self) -> u64 {
        self.latency_ns.load(Ordering::Relaxed) / 1000
    }
}

pub struct UpstreamPool {
    pub name: String,
    pub upstreams: Vec<Arc<UpstreamState>>,
    pub algorithm: crate::config::LoadBalancingAlgorithm,
    pub wrr_counter: AtomicU64,
}

impl UpstreamPool {
    pub fn healthy_upstreams(&self) -> Vec<Arc<UpstreamState>> {
        self.upstreams
            .iter()
            .filter(|u| u.is_available())
            .cloned()
            .collect()
    }

    pub fn all_upstreams(&self) -> &[Arc<UpstreamState>] {
        &self.upstreams
    }

    pub fn select_upstream(&self) -> Option<Arc<UpstreamState>> {
        let healthy = self.healthy_upstreams();
        if healthy.is_empty() {
            return None;
        }
        balancer::select(&healthy, &self.algorithm, &self.wrr_counter)
    }
}

pub fn create_pools(config: &crate::config::Config) -> Vec<Arc<UpstreamPool>> {
    config
        .pools
        .iter()
        .map(|pool_cfg| {
            let upstreams = pool_cfg
                .upstreams
                .iter()
                .map(|u| {
                    Arc::new(UpstreamState::new(
                        pool_cfg.name.clone(),
                        u.url.clone(),
                        u.weight,
                        u.max_connections,
                    ))
                })
                .collect();
            Arc::new(UpstreamPool {
                name: pool_cfg.name.clone(),
                upstreams,
                algorithm: pool_cfg.algorithm.clone(),
                wrr_counter: AtomicU64::new(0),
            })
        })
        .collect()
}
