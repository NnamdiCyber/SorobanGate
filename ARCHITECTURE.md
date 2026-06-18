# SorobanGate Architecture

## Overview

SorobanGate is a high-performance JSON-RPC reverse proxy, load balancer, and API gateway
purpose-built for the Stellar/Soroban ecosystem. It sits in front of a fleet of Soroban RPC
nodes and provides health checking, failover, load balancing, rate limiting, API key
authentication, and response caching.

The entire gateway is a single statically-linked Rust binary using Tokio's async runtime
with Axum as the HTTP framework.

---

## Module Tree

```
sorobangate/
├── src/
│   ├── main.rs                 CLI entry point
│   ├── lib.rs                  Re-exports all modules
│   ├── config/
│   │   ├── mod.rs              Config structs & deserialization
│   │   └── validate.rs         Config validation
│   ├── server/
│   │   ├── mod.rs              AppState, router setup, middleware stack
│   │   ├── proxy.rs            Core request proxy handler
│   │   └── admin.rs            Admin API handlers
│   ├── pool/
│   │   ├── mod.rs              UpstreamState, UpstreamPool
│   │   ├── balancer.rs         WRR / LeastConnections / Random
│   │   ├── health.rs           Background health check loop
│   │   └── circuit_breaker.rs  Circuit breaker state machine
│   ├── routing/
│   │   └── mod.rs              Method → pool routing
│   ├── cache/
│   │   ├── mod.rs              Cache trait, cache key computation
│   │   ├── memory.rs           In-memory moka-backed cache
│   │   └── redis.rs            Redis cache (stub)
│   ├── rate_limit/
│   │   ├── mod.rs              RateLimiter trait
│   │   ├── token_bucket.rs     In-memory token bucket
│   │   └── redis.rs            Distributed rate limiter (stub)
│   ├── auth/
│   │   ├── mod.rs              Key extraction & verification
│   │   └── store/
│   │       ├── mod.rs          Dispatch
│   │       ├── sqlite.rs       SQLite key store
│   │       └── redis.rs        Redis key store (stub)
│   ├── metrics/
│   │   └── mod.rs              Prometheus metrics families
│   └── telemetry/
│       └── mod.rs              OpenTelemetry tracing setup
├── tests/
│   ├── common.rs               Test helpers
│   ├── proxy_test.rs
│   ├── cache_test.rs
│   ├── rate_limit_test.rs
│   ├── failover_test.rs
│   ├── integration/
│   │   └── proxy_test.rs
│   └── fixtures/
│       ├── mock_rpc.rs
│       └── mock_server.rs
├── benches/
│   └── gateway_bench.rs        Criterion benchmarks
├── config/
│   ├── sorobangate.example.toml
│   └── grafana-dashboard.json
├── deploy/
│   ├── docker-compose.yml
│   ├── kubernetes/deployment.yaml
│   └── systemd/sorobangate.service
├── Dockerfile
├── Cargo.toml
└── sorobangate.toml
```

---

## Request Lifecycle

```
Client Request
      │
      ▼
┌───────────────────────────────┐
│      axum ::serve()           │  TCP listener on config.server.bind
│   (TLS via axum-server+rustls)│  Optional TLS termination
└──────────────┬────────────────┘
               │
               ▼
┌───────────────────────────────┐
│      Middleware Stack          │
│                                │
│  1. TraceLayer (tower-http)   │  Request/response logging
│  2. gateway_middleware         │  Debug-level method + URI + status
│  3. auth_middleware            │  Extract API key, verify (Argon2)
│                                │  Insert AuthTier + AuthKeyId
│  4. rate_limit_middleware      │  Token bucket check (per-tier or per-IP)
│  5. cache_middleware           │  Passthrough; cache hit checked below
└──────────────┬────────────────┘
               │
               ▼
┌───────────────────────────────┐
│      proxy_handler             │
│                                │
│  1. Generate request_id (UUID) │
│  2. Read request body          │
│  3. Extract JSON-RPC method    │
│  4. Route: method → pool       │
│  5. Cache lookup (SHA-256 key) │
│     → return cached if hit     │
│  6. Select upstream (balancer) │
│  7. Forward via hyper::Client  │
│     → on failure: circuit      │
│       breaker record, 502/504  │
│     → on success: cache,       │
│       record metrics, return   │
└────────────────────────────────┘
```

---

## Core Components

### Config (`src/config/`)

The `Config` struct is the root deserialized from TOML. It aggregates:

| Struct | Purpose |
|---|---|
| `ServerConfig` | Bind addresses, log level/format, timeouts, max connections, worker threads |
| `TlsConfig` | Optional TLS cert/key paths and min version |
| `PoolConfig` | Named upstream pool with algorithm and upstream list |
| `UpstreamConfig` | Single upstream URL + weight + max connections |
| `HealthCheckConfig` | Interval, timeout, thresholds, JSON-RPC method |
| `RoutingConfig` | Default pool + method-to-pool rules |
| `RateLimitConfig` | Enable/disable, store backend, IP fallback RPS/burst |
| `AuthConfig` | Enable/disable, allow_unauthenticated, key_store, db_path, tiers |
| `CacheConfig` | Enable/disable, backend, max_memory_mb, redis_url, TTL rules |
| `TelemetryConfig` | Metrics/tracing enable flags, OTLP endpoint |

All values can be overridden by environment variables (`SOROBANGATE__SECTION__KEY`).

### Server (`src/server/`)

**AppState** is the shared state injected into all Axum handlers via `State<Arc<AppState>>`:

```rust
pub struct AppState {
    pub config: Arc<Config>,
    pub pools: Vec<UpstreamPool>,
    pub router: Router,
    pub cache: Option<CacheDispatch>,
    pub rate_limiter: Option<RateLimiterDispatch>,
    pub key_store: Option<KeyStoreDispatch>,
    pub auth_tiers: Vec<AuthTierConfig>,
    pub metrics: MetricsHandle,
    pub start_time: Instant,
}
```

The middleware stack is assembled in `build_router()` in this order:

1. **`tower_http::TraceLayer`** — logs method, URI, status, latency
2. **`gateway_middleware`** — custom debug logging
3. **`auth_middleware`** — API key extraction + Argon2 verification
4. **`rate_limit_middleware`** — token bucket rate limiting per tier/IP
5. **`cache_middleware`** — currently passes through; cache logic is in proxy_handler

### Pool (`src/pool/`)

**UpstreamState** represents a single upstream node:

```rust
pub struct UpstreamState {
    pub url: Url,
    pub weight: u32,
    pub max_connections: Option<u32>,
    state: Mutex<UpstreamMutableState>,
    active_connections: AtomicU32,
    latency_ns: AtomicU64,
}

struct UpstreamMutableState {
    pub health: HealthStatus,
    pub circuit_breaker: CircuitBreaker,
    pub consecutive_successes: u32,
    pub consecutive_failures: u32,
}
```

**UpstreamPool** is a named group with a balancer algorithm:

```rust
pub struct UpstreamPool {
    pub name: String,
    pub upstreams: Vec<Arc<UpstreamState>>,
    algorithm: LoadBalancingAlgorithm,
    wrr_counter: AtomicU64,
}
```

**Three load balancing algorithms** implement the `LoadBalancer` trait:

| Algorithm | Strategy |
|---|---|
| `WeightedRoundRobin` | Distributes proportional to weight using atomic counter |
| `LeastConnections` | Picks upstream with fewest active connections |
| `Random` | Picks from healthy set uniformly |

**Health Check Loop** runs in a background Tokio task:

- Every `interval_ms`, probes all upstreams concurrently
- Tracks `consecutive_successes` / `consecutive_failures`
- Crosses `healthy_threshold` → marks Healthy
- Crosses `unhealthy_threshold` → marks Unhealthy
- Circuit breaker uses a separate failure threshold + cooldown timer

**Circuit Breaker** states:

| State | Behavior |
|---|---|
| `Closed` | Normal operation; failures increment counter |
| `Open` | No traffic routed; timer counts down cooldown |
| `HalfOpen` | Allows one probe request; success → Closed, failure → Open |

### Routing (`src/routing/`)

Routes JSON-RPC methods to upstream pools:

```rust
pub struct Router {
    method_map: HashMap<String, String>,
    default_pool: String,
}
```

Built from `[[routing.rules]]` — each rule maps a list of method names to a pool.
Unmatched methods fall through to `default_pool`.

### Cache (`src/cache/`)

**Cache** trait with two implementations (in-memory complete, Redis stubbed):

```rust
#[async_trait]
pub trait Cache: Send + Sync {
    async fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    async fn set(&self, key: &[u8], value: Vec<u8>, ttl: Duration);
    async fn delete(&self, key: &[u8]);
    async fn flush(&self);
    async fn stats(&self) -> CacheStats;
}
```

**MemoryCache** uses `moka::sync::Cache<u64, CachedValue>`:

- 64-bit hash keys (SipHash-2-4 via `moka` internally)
- `CachedValue` stores raw bytes + `Instant` expiry
- Custom expiry check on read (moka TTL is an upper bound; we enforce exact expiry ourselves)
- Atomic `CacheStats` for hit/miss/size/eviction counts

**Cache key** is computed by `compute_cache_key()`:

1. Parse the JSON-RPC body
2. Strip `id` and `jsonrpc` fields (non-semantic)
3. Normalize parameter order (sort keys)
4. SHA-256 hash the canonical JSON

### Rate Limiter (`src/rate_limit/`)

**TokenBucketRateLimiter** implements the `RateLimiter` trait:

```rust
#[async_trait]
pub trait RateLimiter: Send + Sync {
    async fn check_rate(&self, key: &str, rps: u32, burst: u32) -> Result<(), RateLimited>;
}
```

Token bucket algorithm:

- Each key has a bucket with `burst` capacity
- Tokens refill at `rps` per second (using elapsed time since last refill)
- On request: if tokens >= 1, consume and allow; else deny with 429
- Buckets cleaned up after inactivity (via idleness + periodic sweep)

### Auth (`src/auth/`)

**Key extraction** tries three sources in order:

1. `Authorization: Bearer <key>` header
2. `X-API-Key: <key>` header
3. `?api_key=<key>` query parameter

**SqliteKeyStore** implements the `KeyStore` trait:

- Keys stored with: key_id (SHA-256 of prefix), Argon2id hash, tier label, revoke flag, created_at
- Plaintext secret shown only once on creation
- Lookup: iterate all keys, verify with Argon2

**Key format:** `sgk_live_<base62 random>` / `sgk_test_<base62 random>`

### Metrics (`src/metrics/`)

Eight Prometheus metric families:

| Metric | Type | Labels |
|---|---|---|
| `sorobangate_requests_total` | Counter | method, pool, status, cached |
| `sorobangate_request_duration_seconds` | Histogram | method, pool |
| `sorobangate_upstream_health` | Gauge | upstream, pool |
| `sorobangate_upstream_latency_seconds` | Histogram | upstream, pool |
| `sorobangate_cache_hits_total` | Counter | method |
| `sorobangate_cache_misses_total` | Counter | method |
| `sorobangate_rate_limit_rejections_total` | Counter | tier, reason |
| `sorobangate_circuit_breaker_state` | Gauge | upstream |
| `sorobangate_active_connections` | Gauge | pool |

Exported at `/metrics` on `config.server.metrics_bind`.

---

## Key Design Decisions

1. **JSON-RPC-native** — Extracts the RPC method to route, cache, and rate-limit at method granularity rather than treating all requests uniformly.

2. **Pluggable backends via traits** — `Cache`, `RateLimiter`, and `KeyStore` are traits with dispatch enums. In-memory/SQLite are implemented; Redis stubs are ready for completion.

3. **Middleware-stacked architecture** — Axum middleware layers cleanly separate auth, rate limiting, caching, and logging. Each is independently configurable.

4. **Concurrency model** — Shared state via `Arc + Mutex` for mutable health/circuit-breaker state, atomics for connection counts and cache stats. Moka cache provides internal concurrent access.

5. **Health checks + circuit breakers** — Two independent mechanisms: health checks use consecutive success/failure counts; circuit breakers use a fault threshold + cooldown timer.

---

## Testing Strategy

### Unit tests (`#[cfg(test)]` in source files)

| Module | Coverage |
|---|---|
| `pool/health.rs` | Health transitions, filtering, circuit breaker integration |
| `pool/circuit_breaker.rs` | Full state machine (Closed/Open/HalfOpen transitions) |
| `pool/balancer.rs` | WRR distribution, LC picking, Random selection, edge cases |
| `cache/memory.rs` | Hit/miss, expiry, flush, stats, key determinism |
| `rate_limit/token_bucket.rs` | Burst, refill, independent keys, IP fallback |
| `auth/mod.rs` | Key extraction (Bearer, X-API-Key, query param) |
| `auth/store/sqlite.rs` | Create, lookup, revoke, list |

### Integration tests (`tests/`)

- Mock servers on random ports for isolated testing
- `tests/common.rs` provides `start_mock()` and `start_gateway()` helpers
- Tests cover: basic proxying, caching (upstream called only once), rate limiting (burst + 429), failover (unreachable upstream, recovery)

### Benchmarks (`benches/gateway_bench.rs`)

Criterion benchmarks for proxy throughput and cache hit performance.

---

## Deployment

- **Docker** — Multi-stage distroless build (`gcr.io/distroless/cc-debian12`)
- **Docker Compose** — SorobanGate + Redis + Prometheus + Grafana
- **Kubernetes** — Deployment with 3 replicas, liveness/readiness probes, ConfigMap
- **systemd** — Hardened unit with `CAP_NET_BIND_SERVICE`, `LimitNOFILE=100000`

---

## Dependencies

| Crate | Purpose |
|---|---|
| **tokio** | Async runtime, TCP listeners, timers |
| **axum** | HTTP server, router, middleware, state |
| **hyper** | HTTP client for upstream proxying |
| **moka** | High-performance concurrent in-memory cache |
| **rusqlite** | SQLite key store for API keys |
| **argon2** | Key hashing and verification |
| **prometheus-client** | Metrics families and exposition |
| **tracing / tracing-subscriber** | Structured logging |
| **serde / toml** | Config deserialization |
| **clap** | CLI argument parsing |
| **sha2** | Cache key hashing |
| **uuid** | Request ID generation |
| **axum-server** | TLS termination (optional) |

---

## Redis Backends (Stubs)

The Redis backends for cache, rate limiting, and key store are defined but not yet
implemented. They follow the same trait interfaces as the in-memory/SQLite backends:

| File | Trait | Status |
|---|---|---|
| `src/cache/redis.rs` | `Cache` | Stub |
| `src/rate_limit/redis.rs` | `RateLimiter` | Stub |
| `src/auth/store/redis.rs` | `KeyStore` | Stub |

The dispatch enums (`CacheDispatch`, `RateLimiterDispatch`, `KeyStoreDispatch`) are
ready to route to these implementations once completed.
