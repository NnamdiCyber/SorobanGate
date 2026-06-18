use prometheus_client::{
    encoding::text::encode,
    metrics::{counter::Counter, family::Family, gauge::Gauge, histogram::Histogram},
    registry::Registry,
};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct RequestLabels {
    pub method: String,
    pub pool: String,
    pub status: String,
    pub cached: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct MethodPoolLabels {
    pub method: String,
    pub pool: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct UpstreamLabels {
    pub upstream: String,
    pub pool: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct MethodLabels {
    pub method: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct TierReasonLabels {
    pub tier: String,
    pub reason: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct PoolLabels {
    pub pool: String,
}

#[derive(Clone)]
pub struct Metrics {
    pub requests_total: Family<RequestLabels, Counter>,
    pub request_duration_seconds: Family<MethodPoolLabels, Histogram>,
    pub upstream_health: Family<UpstreamLabels, Gauge>,
    pub upstream_latency_seconds: Family<UpstreamLabels, Histogram>,
    pub cache_hits_total: Family<MethodLabels, Counter>,
    pub cache_misses_total: Family<MethodLabels, Counter>,
    pub rate_limit_rejections_total: Family<TierReasonLabels, Counter>,
    pub circuit_breaker_state: Family<UpstreamLabels, Gauge>,
    pub active_connections: Family<PoolLabels, Gauge>,
}

impl Metrics {
    pub fn new() -> (Self, Registry) {
        let mut registry = Registry::default();

        let duration_buckets = || {
            Histogram::new(
                [0.0005, 0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]
                    .into_iter(),
            )
        };

        let requests_total = Family::<RequestLabels, Counter>::default();
        registry.register(
            "sorobangate_requests",
            "Total requests",
            requests_total.clone(),
        );

        let request_duration_seconds =
            Family::<MethodPoolLabels, Histogram>::new_with_constructor(duration_buckets);
        registry.register(
            "sorobangate_request_duration_seconds",
            "Request duration in seconds",
            request_duration_seconds.clone(),
        );

        let upstream_health = Family::<UpstreamLabels, Gauge>::default();
        registry.register(
            "sorobangate_upstream_health",
            "Upstream health state (1=healthy, 0=unhealthy)",
            upstream_health.clone(),
        );

        let upstream_latency_seconds =
            Family::<UpstreamLabels, Histogram>::new_with_constructor(duration_buckets);
        registry.register(
            "sorobangate_upstream_latency_seconds",
            "Upstream response latency in seconds",
            upstream_latency_seconds.clone(),
        );

        let cache_hits_total = Family::<MethodLabels, Counter>::default();
        registry.register(
            "sorobangate_cache_hits",
            "Cache hits by method",
            cache_hits_total.clone(),
        );

        let cache_misses_total = Family::<MethodLabels, Counter>::default();
        registry.register(
            "sorobangate_cache_misses",
            "Cache misses by method",
            cache_misses_total.clone(),
        );

        let rate_limit_rejections_total = Family::<TierReasonLabels, Counter>::default();
        registry.register(
            "sorobangate_rate_limit_rejections",
            "Rate limit rejections",
            rate_limit_rejections_total.clone(),
        );

        let circuit_breaker_state = Family::<UpstreamLabels, Gauge>::default();
        registry.register(
            "sorobangate_circuit_breaker_state",
            "Circuit breaker state (0=closed, 1=open, 2=half-open)",
            circuit_breaker_state.clone(),
        );

        let active_connections = Family::<PoolLabels, Gauge>::default();
        registry.register(
            "sorobangate_active_connections",
            "Active connections per pool",
            active_connections.clone(),
        );

        (
            Self {
                requests_total,
                request_duration_seconds,
                upstream_health,
                upstream_latency_seconds,
                cache_hits_total,
                cache_misses_total,
                rate_limit_rejections_total,
                circuit_breaker_state,
                active_connections,
            },
            registry,
        )
    }
}

pub struct MetricsHandle {
    pub metrics: Metrics,
    registry: Arc<Mutex<Registry>>,
}

impl Default for MetricsHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsHandle {
    pub fn new() -> Self {
        let (metrics, registry) = Metrics::new();
        Self {
            metrics,
            registry: Arc::new(Mutex::new(registry)),
        }
    }

    pub async fn render(&self) -> String {
        let mut buf = String::new();
        let registry = self.registry.lock().await;
        encode(&mut buf, &registry).unwrap_or_default();
        buf
    }
}
