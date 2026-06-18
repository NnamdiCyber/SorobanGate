use std::sync::Arc;
use std::time::Duration;

use http_body_util::Full;
use hyper::body::Bytes;
use serde_json::json;
use tokio::time::Instant;

use crate::config::Config;
use crate::pool::{HealthStatus, UpstreamPool};

pub async fn run_health_checks(
    pools: Vec<Arc<UpstreamPool>>,
    config: Arc<Config>,
    skip_initial: bool,
) {
    if skip_initial {
        // Mark all upstreams healthy so the gateway can serve traffic immediately
        for pool in &pools {
            for upstream in &pool.upstreams {
                let mut state = upstream.mutable.lock().unwrap();
                state.health = HealthStatus::Healthy;
            }
        }
    } else {
        run_health_check_cycle(&pools, &config).await;
    }

    let interval_dur = Duration::from_millis(config.health_check.interval_ms);
    let mut interval = tokio::time::interval(interval_dur);
    interval.tick().await;

    loop {
        interval.tick().await;
        run_health_check_cycle(&pools, &config).await;
    }
}

async fn run_health_check_cycle(pools: &[Arc<UpstreamPool>], config: &Config) {
    let mut handles = Vec::new();
    for pool in pools {
        for upstream in &pool.upstreams {
            let upstream = upstream.clone();
            let cfg = config.clone();
            handles.push(tokio::spawn(async move {
                check_upstream(upstream, &cfg).await;
            }));
        }
    }
    for h in handles {
        let _ = h.await;
    }
}

async fn check_upstream(upstream: Arc<crate::pool::UpstreamState>, config: &Config) {
    let body_bytes = match serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": config.health_check.method,
        "params": []
    })) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = %e, "Failed to serialize health check body");
            return;
        }
    };

    let uri: hyper::Uri = match upstream.url.parse() {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!(
                url = %upstream.url,
                pool = %upstream.pool_name,
                error = %e,
                "Invalid upstream URL in health check"
            );
            return;
        }
    };

    let req = match hyper::Request::post(&uri)
        .header("Content-Type", "application/json")
        .body(Full::<Bytes>::from(body_bytes))
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                url = %upstream.url,
                pool = %upstream.pool_name,
                error = %e,
                "Failed to build health check request"
            );
            return;
        }
    };

    let connector = hyper_util::client::legacy::connect::HttpConnector::new();
    let client: hyper_util::client::legacy::Client<
        hyper_util::client::legacy::connect::HttpConnector,
        Full<Bytes>,
    > = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build(connector);

    let start = Instant::now();
    let timeout = Duration::from_millis(config.health_check.timeout_ms);

    let result = tokio::time::timeout(timeout, client.request(req)).await;
    let elapsed = start.elapsed();

    let mut state = upstream.mutable.lock().unwrap();

    state.circuit_breaker.try_half_open();

    match result {
        Ok(Ok(response)) if response.status().is_success() => {
            state.consecutive_successes += 1;
            state.consecutive_failures = 0;
            upstream.latency_ns.store(
                elapsed.as_nanos() as u64,
                std::sync::atomic::Ordering::Relaxed,
            );

            let prev_health = state.health;
            if state.consecutive_successes >= config.health_check.healthy_threshold {
                state.health = HealthStatus::Healthy;
            }

            if prev_health != HealthStatus::Healthy && state.health == HealthStatus::Healthy {
                tracing::info!(
                    url = %upstream.url,
                    pool = %upstream.pool_name,
                    consecutive_successes = state.consecutive_successes,
                    latency_us = elapsed.as_micros(),
                    "Upstream became healthy"
                );
            }
        }
        _ => {
            state.consecutive_failures += 1;
            state.consecutive_successes = 0;

            let prev_health = state.health;
            if state.consecutive_failures >= config.health_check.unhealthy_threshold {
                state.health = HealthStatus::Unhealthy;
            }

            if prev_health != HealthStatus::Unhealthy && state.health == HealthStatus::Unhealthy {
                tracing::warn!(
                    url = %upstream.url,
                    pool = %upstream.pool_name,
                    consecutive_failures = state.consecutive_failures,
                    "Upstream became unhealthy"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::pool::{HealthStatus, UpstreamPool, UpstreamState};
    use std::sync::atomic::AtomicU64;
    use std::sync::Arc;

    #[test]
    fn test_health_status_transitions() {
        let upstream = UpstreamState::new(
            "test".to_string(),
            "http://localhost:9999".to_string(),
            1,
            None,
        );

        {
            let state = upstream.mutable.lock().unwrap();
            assert_eq!(state.health, HealthStatus::Unhealthy);
            assert_eq!(state.consecutive_successes, 0);
            assert_eq!(state.consecutive_failures, 0);
        }

        {
            let mut state = upstream.mutable.lock().unwrap();
            state.consecutive_successes += 1;
            state.consecutive_failures = 0;
        }

        {
            let state = upstream.mutable.lock().unwrap();
            assert_eq!(state.consecutive_successes, 1);
            assert_eq!(state.health, HealthStatus::Unhealthy);
        }

        {
            let mut state = upstream.mutable.lock().unwrap();
            state.consecutive_successes += 1;
            state.consecutive_failures = 0;
            if state.consecutive_successes >= 2 {
                state.health = HealthStatus::Healthy;
            }
        }

        {
            let state = upstream.mutable.lock().unwrap();
            assert_eq!(state.health, HealthStatus::Healthy);
        }

        {
            let mut state = upstream.mutable.lock().unwrap();
            state.consecutive_failures += 1;
            state.consecutive_successes = 0;
        }

        {
            let mut state = upstream.mutable.lock().unwrap();
            assert_eq!(state.health, HealthStatus::Healthy);
            state.consecutive_failures += 1;
            state.consecutive_successes = 0;
        }

        {
            let mut state = upstream.mutable.lock().unwrap();
            state.consecutive_failures += 1;
            state.consecutive_successes = 0;
            if state.consecutive_failures >= 3 {
                state.health = HealthStatus::Unhealthy;
            }
        }

        {
            let state = upstream.mutable.lock().unwrap();
            assert_eq!(state.health, HealthStatus::Unhealthy);
        }
    }

    #[test]
    fn test_initial_health_is_unhealthy() {
        let upstream = UpstreamState::new(
            "test".to_string(),
            "http://localhost:9999".to_string(),
            1,
            None,
        );
        let state = upstream.mutable.lock().unwrap();
        assert_eq!(state.health, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_healthy_upstream_filtering() {
        let healthy = Arc::new(UpstreamState::new(
            "test".to_string(),
            "http://healthy:8080".to_string(),
            1,
            None,
        ));
        {
            let mut state = healthy.mutable.lock().unwrap();
            state.health = HealthStatus::Healthy;
        }

        let unhealthy = Arc::new(UpstreamState::new(
            "test".to_string(),
            "http://unhealthy:8080".to_string(),
            1,
            None,
        ));

        let pool = UpstreamPool {
            name: "test".to_string(),
            upstreams: vec![healthy, unhealthy],
            algorithm: crate::config::LoadBalancingAlgorithm::Wrr,
            wrr_counter: AtomicU64::new(0),
        };

        let h = pool.healthy_upstreams();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].url, "http://healthy:8080");
    }

    #[test]
    fn test_circuit_breaker_blocks_healthy_upstream() {
        let upstream = Arc::new(UpstreamState::new(
            "test".to_string(),
            "http://healthy-but-cb-open:8080".to_string(),
            1,
            None,
        ));
        {
            let mut state = upstream.mutable.lock().unwrap();
            state.health = HealthStatus::Healthy;
            state.circuit_breaker.record_failure();
            state.circuit_breaker.record_failure();
            state.circuit_breaker.record_failure();
            state.circuit_breaker.record_failure();
            state.circuit_breaker.record_failure();
            assert!(!state.circuit_breaker.is_available());
        }

        assert!(!upstream.is_available());

        let pool = UpstreamPool {
            name: "test".to_string(),
            upstreams: vec![upstream],
            algorithm: crate::config::LoadBalancingAlgorithm::Wrr,
            wrr_counter: AtomicU64::new(0),
        };

        let healthy = pool.healthy_upstreams();
        assert_eq!(healthy.len(), 0);
    }
}
