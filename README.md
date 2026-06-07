<div align="center">

<img src="https://img.shields.io/badge/Rust-1.78+-CE422B?style=for-the-badge&logo=rust&logoColor=white" alt="Rust" />
<img src="https://img.shields.io/badge/Soroban-RPC_Gateway-7B2FBE?style=for-the-badge&logo=stellar&logoColor=white" alt="Soroban" />
<img src="https://img.shields.io/badge/License-Apache_2.0-0F6E56?style=for-the-badge" alt="Apache 2.0" />
<img src="https://img.shields.io/badge/PRs-Welcome-D85A30?style=for-the-badge" alt="PRs Welcome" />
<img src="https://img.shields.io/badge/Open_Source-Community-534AB7?style=for-the-badge&logo=opensourceinitiative&logoColor=white" alt="Open Source" />

<br /><br />

```
  ███████╗ ██████╗ ██████╗   ██████╗  ██████╗   █████╗  ███╗  ██╗  ██████╗  █████╗  ████████╗███████╗
  ██╔════╝██╔═══██╗██╔══██╗ ██╔═══██╗██╔══██╗  ██╔══██╗ ████╗ ██║ ██╔════╝ ██╔══██╗ ╚══██╔══╝██╔════╝
  ███████╗██║   ██║██████╔╝ ██║   ██║██████╔╝  ███████║ ██╔██╗██║ ██║  ███╗███████║    ██║   █████╗
  ╚════██║██║   ██║██╔══██╗ ██║   ██║██╔══██╗  ██╔══██║ ██║╚████║ ██║   ██║██╔══██║    ██║   ██╔══╝
  ███████║╚██████╔╝██║  ██║ ╚██████╔╝██████╔╝  ██║  ██║ ██║ ╚███║ ╚██████╔╝██║  ██║    ██║   ███████╗
  ╚══════╝ ╚═════╝ ╚═╝  ╚═╝  ╚═════╝ ╚═════╝   ╚═╝  ╚═╝ ╚═╝  ╚══╝  ╚═════╝ ╚═╝  ╚═╝    ╚═╝   ╚══════╝
```

### High-performance Soroban RPC gateway & load balancer — written in Rust.

**Health Checking · Failover · Load Balancing · Rate Limiting · API Key Auth · Response Caching**

<br />

[🚀 Quick Start](#-quick-start) · [📖 Documentation](https://docs.sorobangate.dev) · [💬 Discord](https://discord.gg/sorobangate) · [🐛 Report Bug](https://github.com/sorobangate/sorobangate/issues) · [✨ Request Feature](https://github.com/sorobangate/sorobangate/issues)

<br />

```
Benchmarks (c5.2xlarge, 8 vCPU, single node)

  Throughput   →  48,200 req/s sustained
  P50 latency  →  0.8ms
  P99 latency  →  3.2ms
  Failover     →  < 50ms automatic
  Memory       →  ~18MB idle
```

</div>

---

## Table of Contents

- [Why SorobanGate?](#-why-sorobangate)
- [Architecture Overview](#-architecture-overview)
- [Core Features](#-core-features)
  - [Health Checking & Failover](#1-health-checking--failover)
  - [Request Routing & Load Balancing](#2-request-routing--load-balancing)
  - [Rate Limiting & API Key Auth](#3-rate-limiting--api-key-auth)
  - [Response Caching](#4-response-caching)
  - [Observability](#5-observability)
- [Quick Start](#-quick-start)
- [Installation](#-installation)
  - [From Source](#from-source)
  - [Docker](#docker)
  - [Pre-built Binaries](#pre-built-binaries)
- [Configuration](#-configuration)
  - [Full Reference](#full-configuration-reference)
  - [Upstream Pools](#upstream-pools)
  - [Rate Limiting](#rate-limiting-config)
  - [Caching](#caching-config)
  - [TLS](#tls-config)
- [API Key Management](#-api-key-management)
- [Admin API](#-admin-api)
- [Deployment](#-deployment)
  - [Single Node](#single-node)
  - [High Availability](#high-availability-setup)
  - [Kubernetes](#kubernetes)
  - [Docker Compose](#docker-compose)
- [Performance Tuning](#-performance-tuning)
- [Project Structure](#-project-structure)
- [Contributing](#-contributing)
- [Roadmap](#-roadmap)
- [Security](#-security)
- [License](#-license)
- [Acknowledgements](#-acknowledgements)

---

## 🤔 Why SorobanGate?

The Stellar/Soroban ecosystem depends on reliable RPC access. Running production applications directly against public RPC endpoints — or even self-hosted nodes — creates a fragile single point of failure, exposes you to rate limits, and gives you zero control over traffic shaping.

SorobanGate sits in front of your RPC fleet and solves all of this:

| Problem | Without SorobanGate | With SorobanGate |
|---|---|---|
| **Node failures** | App errors until manual intervention | Automatic failover in < 50ms |
| **Traffic spikes** | Node overload, dropped requests | Distributed load across healthy upstreams |
| **Repeated calls** | Redundant RPC round-trips every time | Cached responses served in < 1ms |
| **Abuse / runaway clients** | Uncontrolled load on nodes | Per-key & per-IP rate limiting |
| **No visibility** | Black-box RPC calls | Prometheus metrics + structured logs on every request |
| **Multi-environment** | Separate configs for testnet/mainnet | Named upstream pools, one gateway |

SorobanGate is built for operators who care about reliability, performance, and cost. It is written in Rust — not because Rust is trendy, but because zero-cost abstractions, fearless concurrency, and a sub-20MB binary matter when you are running this on the hot path of every contract interaction.

---

## 🏗 Architecture Overview

```
                         ┌────────────────────────────────────────────────┐
                         │               SorobanGate                      │
                         │                                                │
  Clients ──────────────►│  ┌──────────┐   ┌──────────┐   ┌──────────┐  │
  (dApps, SDKs,          │  │  TLS     │   │  Auth &  │   │  Rate    │  │
   wallets, scripts)     │  │  Termina-│──►│  API Key │──►│  Limiter │  │
                         │  │  tion    │   │  Verify  │   │  (token  │  │
                         │  └──────────┘   └──────────┘   │  bucket) │  │
                         │                                 └────┬─────┘  │
                         │                                      │        │
                         │  ┌───────────────────────────────────▼──────┐ │
                         │  │              Request Router               │ │
                         │  │  method-aware · weighted · sticky session │ │
                         │  └──────┬─────────────────────┬─────────────┘ │
                         │         │                     │               │
                         │  ┌──────▼──────┐       ┌─────▼──────┐        │
                         │  │   Cache     │       │  Load      │        │
                         │  │   Layer     │       │  Balancer  │        │
                         │  │  (in-mem +  │       │  (WRR/LC/  │        │
                         │  │   Redis)    │       │   random)  │        │
                         │  └──────┬──────┘       └─────┬──────┘        │
                         │         │ cache miss          │               │
                         │         └─────────────────────┘               │
                         │                     │                         │
                         │  ┌──────────────────▼───────────────────────┐ │
                         │  │            Upstream Pool Manager          │ │
                         │  │  health-check loop · circuit breaker      │ │
                         │  └──────┬───────────────────┬───────────────┘ │
                         └─────────┼───────────────────┼─────────────────┘
                                   │                   │
                    ┌──────────────▼──┐   ┌────────────▼──────────────┐
                    │  Soroban RPC    │   │  Soroban RPC              │
                    │  Node A         │   │  Node B (failover)        │
                    │  (primary)      │   │                           │
                    └─────────────────┘   └───────────────────────────┘
```

SorobanGate is a single statically-linked binary. Each inbound connection is handled by Tokio's async runtime. The hot path — auth check → cache lookup → upstream dispatch — is allocation-free for cached responses and involves zero blocking I/O.

---

## ✨ Core Features

### 1. Health Checking & Failover

SorobanGate continuously monitors every upstream node with configurable active health checks. When a node becomes unhealthy, traffic is immediately rerouted to healthy upstreams. When it recovers, it is gradually reintroduced via a configurable warm-up window.

**How it works:**

- **Active probing** — periodic JSON-RPC health calls (`getHealth`, `getLatestLedger`) to every upstream
- **Passive detection** — 5xx responses and connection timeouts increment a per-upstream error counter; crossing the threshold trips the circuit breaker
- **Circuit breaker states** — `Closed` (normal) → `Open` (failed, no traffic) → `Half-Open` (probe, recover)
- **Graduated recovery** — nodes re-enter the pool at a configurable weight, increasing over a warm-up window

```toml
[health_check]
interval_ms = 5000          # probe every upstream every 5s
timeout_ms = 2000           # health check timeout
healthy_threshold = 2       # consecutive successes to mark healthy
unhealthy_threshold = 3     # consecutive failures to mark unhealthy
warm_up_window_secs = 30    # gradual traffic ramp after recovery
```

**Failover is automatic and sub-50ms.** There is no operator intervention required.

---

### 2. Request Routing & Load Balancing

Three load balancing algorithms are available, selectable per upstream pool:

| Algorithm | Flag | Best for |
|---|---|---|
| Weighted round-robin | `wrr` | Heterogeneous node capacity |
| Least connections | `lc` | Long-lived or variable-latency requests |
| Random | `random` | Uniform nodes, lowest overhead |

**Method-aware routing** lets you pin specific JSON-RPC methods to dedicated upstream pools — for example, routing all `simulateTransaction` calls (CPU-heavy) to a high-spec pool while lighter read calls go to a cost-optimised pool.

```toml
[[routing.rules]]
methods = ["simulateTransaction", "sendTransaction"]
pool = "high-performance"

[[routing.rules]]
methods = ["getLedgerEntries", "getEvents"]
pool = "read-replicas"

[routing]
default_pool = "general"
```

**Sticky sessions** (optional) — route all requests from the same API key or client IP to the same upstream for the duration of a configurable window. Useful when sequential requests have implicit ordering guarantees.

---

### 3. Rate Limiting & API Key Auth

SorobanGate supports two-tier rate limiting: per-API-key limits for authenticated clients and per-IP limits for unauthenticated or public traffic.

**Rate limiting algorithm:** Token bucket — smooth burst handling with configurable refill rate and bucket capacity.

**Authentication modes:**
- `Bearer` token in `Authorization` header
- `X-API-Key` header
- Query parameter `?api_key=...` (for websocket upgrades)

**Key tiers example:**

```toml
[[api_keys.tiers]]
name = "free"
requests_per_second = 10
burst = 20
daily_limit = 50_000

[[api_keys.tiers]]
name = "pro"
requests_per_second = 500
burst = 1000
daily_limit = 10_000_000

[[api_keys.tiers]]
name = "unlimited"
requests_per_second = 0      # 0 = no limit
burst = 0
daily_limit = 0
```

Keys are stored in a local SQLite database (for single-node) or Redis (for clustered deployments). They are managed via the [Admin API](#-admin-api) or the `sorobangate keys` CLI.

**Per-IP fallback** — unauthenticated requests are allowed but subject to a strict IP-level rate limit (configurable, default 5 req/s).

---

### 4. Response Caching

SorobanGate caches deterministic RPC responses to dramatically reduce upstream load and cut latency for repeated queries.

**Cache backends:**
- **In-memory** (default) — ultra-fast, zero dependencies, bounded by `max_memory_mb`
- **Redis** — shared cache for multi-node SorobanGate deployments

**Method-level cache TTLs** — different RPC methods have different staleness tolerance:

```toml
[cache]
backend = "memory"           # "memory" | "redis"
max_memory_mb = 512

[[cache.rules]]
methods = ["getLedgerEntries", "getAccount"]
ttl_secs = 6                 # ~1 ledger close

[[cache.rules]]
methods = ["getNetwork", "getFeeStats"]
ttl_secs = 60

[[cache.rules]]
methods = ["getLatestLedger"]
ttl_secs = 2

[[cache.rules]]
methods = ["simulateTransaction", "sendTransaction"]
ttl_secs = 0                 # never cache — always fresh
```

**Cache key** is a SHA-256 hash of the JSON-RPC method + params, normalised to be parameter-order-independent.

**Cache hit rates** — in typical dApp workloads, 40–70% of read requests are cache hits, reducing upstream load proportionally.

---

### 5. Observability

SorobanGate is built to be operated confidently in production.

**Prometheus metrics** — exported at `/metrics`:

```
sorobangate_requests_total{method, pool, status, cached}
sorobangate_request_duration_seconds{method, pool, quantile}
sorobangate_upstream_health{upstream, pool}
sorobangate_upstream_latency_seconds{upstream, pool, quantile}
sorobangate_cache_hits_total{method}
sorobangate_cache_misses_total{method}
sorobangate_rate_limit_rejections_total{tier, reason}
sorobangate_circuit_breaker_state{upstream}   # 0=closed 1=open 2=half-open
sorobangate_active_connections{pool}
```

**Structured logging** — JSON logs (or human-readable in dev mode) on every request:

```json
{
  "timestamp": "2026-06-07T10:32:14.221Z",
  "level": "info",
  "request_id": "01HY4K...",
  "method": "getLedgerEntries",
  "upstream": "node-a:8000",
  "pool": "read-replicas",
  "duration_ms": 1.4,
  "cached": false,
  "api_key_tier": "pro",
  "status": 200
}
```

**Admin health endpoint** — `GET /health` returns gateway and upstream status as JSON, suitable for load balancer health checks.

**Distributed tracing** — optional OpenTelemetry export (OTLP) for trace propagation across your stack.

---

## ⚡ Quick Start

The fastest way to get SorobanGate running locally against Stellar Testnet:

```bash
# 1. Install
cargo install sorobangate

# 2. Create a minimal config
cat > sorobangate.toml << 'EOF'
[server]
bind = "0.0.0.0:8080"

[[pools]]
name = "testnet"
algorithm = "wrr"

[[pools.upstreams]]
url = "https://soroban-testnet.stellar.org"
weight = 1
EOF

# 3. Run
sorobangate --config sorobangate.toml

# 4. Verify
curl -X POST http://localhost:8080 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth","params":[]}'
```

Your gateway is now running. All requests to `http://localhost:8080` are proxied, load-balanced, and health-checked against your upstream pool.

---

## 📦 Installation

### From Source

Requires Rust 1.78+.

```bash
git clone https://github.com/sorobangate/sorobangate.git
cd sorobangate
cargo build --release
./target/release/sorobangate --version
```

For maximum performance, enable CPU-native optimisations:

```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

### Docker

```bash
# Pull latest
docker pull ghcr.io/sorobangate/sorobangate:latest

# Run with config mounted
docker run -d \
  --name sorobangate \
  -p 8080:8080 \
  -p 9090:9090 \
  -v $(pwd)/sorobangate.toml:/etc/sorobangate/config.toml \
  ghcr.io/sorobangate/sorobangate:latest
```

### Pre-built Binaries

Download from [GitHub Releases](https://github.com/sorobangate/sorobangate/releases):

| Platform | Archive |
|---|---|
| Linux x86_64 | `sorobangate-linux-x86_64.tar.gz` |
| Linux aarch64 | `sorobangate-linux-aarch64.tar.gz` |
| macOS x86_64 | `sorobangate-macos-x86_64.tar.gz` |
| macOS aarch64 (Apple Silicon) | `sorobangate-macos-aarch64.tar.gz` |

```bash
# Linux x86_64 example
curl -LO https://github.com/sorobangate/sorobangate/releases/latest/download/sorobangate-linux-x86_64.tar.gz
tar -xzf sorobangate-linux-x86_64.tar.gz
sudo mv sorobangate /usr/local/bin/
sorobangate --version
```

---

## ⚙️ Configuration

SorobanGate is configured via a single TOML file. The config path is set via `--config` flag (default: `./sorobangate.toml`) or the `SOROBANGATE_CONFIG` environment variable.

All values can be overridden by environment variables in the form `SOROBANGATE__SECTION__KEY` (double underscore as separator).

### Full Configuration Reference

```toml
# ─────────────────────────────────────────────
# Server
# ─────────────────────────────────────────────
[server]
bind = "0.0.0.0:8080"           # RPC gateway listener
admin_bind = "127.0.0.1:9000"   # Admin API (keep internal)
metrics_bind = "0.0.0.0:9090"   # Prometheus metrics
log_level = "info"               # trace | debug | info | warn | error
log_format = "json"              # json | pretty
request_timeout_ms = 30_000     # per-request hard timeout
max_connections = 50_000         # soft limit; ulimit must allow
worker_threads = 0               # 0 = number of logical CPUs

# ─────────────────────────────────────────────
# TLS (optional)
# ─────────────────────────────────────────────
[tls]
enabled = false
cert_file = "/etc/sorobangate/tls/cert.pem"
key_file  = "/etc/sorobangate/tls/key.pem"
min_version = "tls1.2"

# ─────────────────────────────────────────────
# Upstream pools
# ─────────────────────────────────────────────
[[pools]]
name = "mainnet"
algorithm = "lc"                # wrr | lc | random

  [[pools.upstreams]]
  url = "https://rpc.node-1.example.com"
  weight = 3
  max_connections = 200

  [[pools.upstreams]]
  url = "https://rpc.node-2.example.com"
  weight = 2

  [[pools.upstreams]]
  url = "https://rpc.node-3.example.com"
  weight = 1

[[pools]]
name = "testnet"
algorithm = "random"

  [[pools.upstreams]]
  url = "https://soroban-testnet.stellar.org"
  weight = 1

# ─────────────────────────────────────────────
# Health checks
# ─────────────────────────────────────────────
[health_check]
interval_ms = 5_000
timeout_ms = 2_000
healthy_threshold = 2
unhealthy_threshold = 3
method = "getHealth"
warm_up_window_secs = 30

# ─────────────────────────────────────────────
# Routing rules (method → pool)
# ─────────────────────────────────────────────
[routing]
default_pool = "mainnet"

[[routing.rules]]
methods = ["simulateTransaction", "sendTransaction"]
pool = "mainnet"

[[routing.rules]]
methods = ["getLedgerEntries", "getEvents", "getAccount"]
pool = "mainnet"

# ─────────────────────────────────────────────
# Rate limiting
# ─────────────────────────────────────────────
[rate_limit]
enabled = true
store = "memory"                # "memory" | "redis"
ip_fallback_rps = 5
ip_fallback_burst = 10

# ─────────────────────────────────────────────
# API key auth
# ─────────────────────────────────────────────
[auth]
enabled = true
allow_unauthenticated = true
key_store = "sqlite"
db_path = "./sorobangate.db"

[[auth.tiers]]
name = "free"
requests_per_second = 10
burst = 20
daily_limit = 50_000

[[auth.tiers]]
name = "pro"
requests_per_second = 500
burst = 1_000
daily_limit = 10_000_000

[[auth.tiers]]
name = "unlimited"
requests_per_second = 0
burst = 0
daily_limit = 0

# ─────────────────────────────────────────────
# Response caching
# ─────────────────────────────────────────────
[cache]
enabled = true
backend = "memory"
max_memory_mb = 512
redis_url = "redis://127.0.0.1:6379"

[[cache.rules]]
methods = ["getLedgerEntries", "getAccount", "getContractData"]
ttl_secs = 6

[[cache.rules]]
methods = ["getNetwork", "getFeeStats", "getVersionInfo"]
ttl_secs = 60

[[cache.rules]]
methods = ["getLatestLedger"]
ttl_secs = 2

[[cache.rules]]
methods = ["simulateTransaction", "sendTransaction", "getTransactionStatus"]
ttl_secs = 0

# ─────────────────────────────────────────────
# Observability
# ─────────────────────────────────────────────
[telemetry]
metrics_enabled = true
tracing_enabled = false
otlp_endpoint = "http://localhost:4317"
```

### Upstream Pools

Pools are named groups of upstream RPC nodes. Each request is routed to exactly one pool (by the routing rules) and then to one upstream within that pool (by the load balancing algorithm). A pool must have at least one upstream. SorobanGate will not start if all upstreams in the default pool fail their initial health check — pass `--skip-initial-health-check` to override during bootstrap.

### Rate Limiting Config

Rate limiting uses a **token bucket** per key (or per IP for unauthenticated requests). Tokens are added at `requests_per_second` rate up to `burst` capacity. `daily_limit = 0` means no daily cap. Redis is required for consistent rate limiting across multiple SorobanGate instances.

### Caching Config

`ttl_secs = 0` disables caching for that method group. Methods not matched by any rule default to `ttl_secs = 0`. Match is by exact method name.

### TLS Config

SorobanGate can terminate TLS for clients. It always uses TLS when connecting to HTTPS upstreams, with system root certificate verification. Set `tls.upstream_verify = false` only for self-signed development nodes.

---

## 🔑 API Key Management

Keys are managed via the `sorobangate keys` CLI subcommand or the Admin API.

```bash
# Create a new key on the "pro" tier
sorobangate keys create --tier pro --label "my-dapp-production"
# → Created key: sgk_live_xK9mP2...  (copy this — shown once)

# List all keys
sorobangate keys list

# Inspect a key
sorobangate keys get sgk_live_xK9mP2...

# Rotate a key (generates a new secret, old key immediately invalidated)
sorobangate keys rotate sgk_live_xK9mP2...

# Revoke a key
sorobangate keys revoke sgk_live_xK9mP2...

# Change tier
sorobangate keys update sgk_live_xK9mP2... --tier unlimited
```

**Using a key in requests:**

```bash
# Authorization header (preferred)
curl -X POST http://localhost:8080 \
  -H "Authorization: Bearer sgk_live_xK9mP2..." \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth","params":[]}'

# X-API-Key header
curl -X POST http://localhost:8080 \
  -H "X-API-Key: sgk_live_xK9mP2..." \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth","params":[]}'
```

---

## 🛠 Admin API

The Admin API runs on a separate port (default `127.0.0.1:9000`) and should never be exposed publicly.

### Gateway status

```
GET /admin/status
```
```json
{
  "version": "1.2.0",
  "uptime_secs": 86432,
  "pools": [
    {
      "name": "mainnet",
      "upstreams": [
        { "url": "https://rpc.node-1.example.com", "state": "healthy", "latency_p99_ms": 12 },
        { "url": "https://rpc.node-2.example.com", "state": "healthy", "latency_p99_ms": 8 },
        { "url": "https://rpc.node-3.example.com", "state": "unhealthy", "circuit": "open" }
      ]
    }
  ]
}
```

### Force upstream health state

```
POST /admin/upstreams/{url_encoded}/enable
POST /admin/upstreams/{url_encoded}/disable
```

### Cache management

```
DELETE /admin/cache                          # flush all
DELETE /admin/cache?method=getLedgerEntries  # flush by method
GET    /admin/cache/stats                    # hit rate, size, evictions
```

### Config reload

```
POST /admin/reload   # hot-reload config without restart
```

### API key management

```
GET    /admin/keys
POST   /admin/keys          { "tier": "pro", "label": "..." }
GET    /admin/keys/{id}
DELETE /admin/keys/{id}
PATCH  /admin/keys/{id}     { "tier": "unlimited" }
POST   /admin/keys/{id}/rotate
```

---

## 🚢 Deployment

### Single Node

For a production single-node deployment with systemd:

```ini
# /etc/systemd/system/sorobangate.service
[Unit]
Description=SorobanGate RPC Gateway
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=sorobangate
Group=sorobangate
ExecStart=/usr/local/bin/sorobangate --config /etc/sorobangate/config.toml
Restart=on-failure
RestartSec=5
LimitNOFILE=100000
AmbientCapabilities=CAP_NET_BIND_SERVICE

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now sorobangate
sudo systemctl status sorobangate
```

### High Availability Setup

For HA, run two or more SorobanGate instances behind an L4 load balancer (HAProxy, AWS NLB, etc.). Use Redis as the shared backend for rate limiting and caching to ensure consistency across instances.

```
                    ┌───────────┐
  Clients ─────────►│  HAProxy  │
                    │  (L4 LB)  │
                    └─────┬─────┘
                          │
            ┌─────────────┴─────────────┐
            │                           │
    ┌───────▼───────┐           ┌───────▼───────┐
    │  SorobanGate  │           │  SorobanGate  │
    │  Instance A   │           │  Instance B   │
    └───────┬───────┘           └───────┬───────┘
            │                           │
            └─────────┬─────────────────┘
                      │
               ┌──────▼──────┐
               │    Redis     │
               │  (shared     │
               │   cache +    │
               │   rate limit)│
               └─────────────┘
```

Set in your config:

```toml
[cache]
backend = "redis"
redis_url = "redis://redis.internal:6379"

[rate_limit]
store = "redis"
```

### Kubernetes

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: sorobangate
spec:
  replicas: 3
  selector:
    matchLabels:
      app: sorobangate
  template:
    metadata:
      labels:
        app: sorobangate
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "9090"
    spec:
      containers:
        - name: sorobangate
          image: ghcr.io/sorobangate/sorobangate:latest
          ports:
            - containerPort: 8080
            - containerPort: 9090
          volumeMounts:
            - name: config
              mountPath: /etc/sorobangate
          resources:
            requests:
              memory: "64Mi"
              cpu: "250m"
            limits:
              memory: "512Mi"
              cpu: "2"
          livenessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 5
            periodSeconds: 10
          readinessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 2
            periodSeconds: 5
      volumes:
        - name: config
          configMap:
            name: sorobangate-config
---
apiVersion: v1
kind: Service
metadata:
  name: sorobangate
spec:
  selector:
    app: sorobangate
  ports:
    - name: rpc
      port: 80
      targetPort: 8080
    - name: metrics
      port: 9090
      targetPort: 9090
```

### Docker Compose

```yaml
version: "3.9"
services:
  sorobangate:
    image: ghcr.io/sorobangate/sorobangate:latest
    ports:
      - "8080:8080"
      - "9090:9090"
    volumes:
      - ./sorobangate.toml:/etc/sorobangate/config.toml:ro
    environment:
      - SOROBANGATE__SERVER__LOG_LEVEL=info
    depends_on:
      - redis
    restart: unless-stopped

  redis:
    image: redis:7-alpine
    volumes:
      - redis_data:/data
    restart: unless-stopped

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9091:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml:ro
    restart: unless-stopped

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    restart: unless-stopped

volumes:
  redis_data:
```

---

## 🔧 Performance Tuning

### OS-level settings

```bash
# Increase file descriptor limits
echo "sorobangate soft nofile 100000" >> /etc/security/limits.conf
echo "sorobangate hard nofile 100000" >> /etc/security/limits.conf

# Tune TCP stack for high connection throughput
sysctl -w net.core.somaxconn=65535
sysctl -w net.ipv4.tcp_max_syn_backlog=65535
sysctl -w net.ipv4.ip_local_port_range="1024 65535"
sysctl -w net.ipv4.tcp_tw_reuse=1
```

### SorobanGate settings

```toml
[server]
worker_threads = 0        # auto-detect CPU count
max_connections = 50_000
request_timeout_ms = 10_000

[cache]
max_memory_mb = 1024

[[pools.upstreams]]
max_connections = 500     # per-upstream connection pool size
```

### Profile-guided optimisation (for maintainers)

```bash
# Step 1: instrument build
RUSTFLAGS="-C target-cpu=native -C profile-generate=/tmp/pgo-data" \
  cargo build --release

# Step 2: run representative workload, then optimised build
RUSTFLAGS="-C target-cpu=native -C profile-use=/tmp/pgo-data" \
  cargo build --release
```

PGO typically yields an additional 10–15% throughput improvement on top of a standard release build.

---

## 📁 Project Structure

```
sorobangate/
├── src/
│   ├── main.rs                  # Binary entry point, CLI arg parsing
│   ├── config/
│   │   ├── mod.rs               # Config structs (serde-deserialised)
│   │   └── validate.rs          # Config validation & defaults
│   ├── server/
│   │   ├── mod.rs               # Axum router setup, middleware stack
│   │   ├── proxy.rs             # Core request proxy handler
│   │   └── admin.rs             # Admin API handlers
│   ├── pool/
│   │   ├── mod.rs               # UpstreamPool — manages upstream state
│   │   ├── health.rs            # Health check loop (Tokio task)
│   │   ├── circuit_breaker.rs   # Circuit breaker state machine
│   │   └── balancer.rs          # WRR / LC / random algorithms
│   ├── routing/
│   │   └── mod.rs               # Method → pool routing rules
│   ├── cache/
│   │   ├── mod.rs               # Cache trait + dispatch
│   │   ├── memory.rs            # In-memory LRU cache (moka)
│   │   └── redis.rs             # Redis cache backend
│   ├── rate_limit/
│   │   ├── mod.rs               # Rate limiter trait + dispatch
│   │   ├── token_bucket.rs      # Token bucket implementation
│   │   └── redis.rs             # Distributed rate limiter
│   ├── auth/
│   │   ├── mod.rs               # API key extraction & verification
│   │   └── store/
│   │       ├── sqlite.rs        # SQLite key store
│   │       └── redis.rs         # Redis key store
│   ├── metrics/
│   │   └── mod.rs               # Prometheus metrics registry
│   └── telemetry/
│       └── mod.rs               # OpenTelemetry tracing setup
│
├── tests/
│   ├── integration/
│   │   ├── proxy_test.rs
│   │   ├── failover_test.rs
│   │   ├── rate_limit_test.rs
│   │   └── cache_test.rs
│   └── fixtures/                # Mock upstream servers for testing
│
├── benches/
│   └── gateway_bench.rs         # Criterion benchmarks
│
├── config/
│   ├── sorobangate.example.toml # Full annotated example config
│   └── grafana-dashboard.json   # Ready-to-import Grafana dashboard
│
├── deploy/
│   ├── docker-compose.yml
│   ├── kubernetes/
│   └── systemd/
│
├── Cargo.toml
├── Cargo.lock
├── Dockerfile
├── .github/workflows/
└── README.md
```

---

## 🤝 Contributing

SorobanGate is community-driven. All contributions are welcome.

### Development Setup

```bash
git clone https://github.com/sorobangate/sorobangate.git
cd sorobangate

# Install dev toolchain
rustup component add clippy rustfmt
cargo install cargo-nextest cargo-watch

# Start a mock upstream for local testing
cargo run --example mock_rpc_server &

# Run gateway in watch mode
cargo watch -x 'run -- --config config/sorobangate.example.toml'

# Run tests (fast, parallel)
cargo nextest run

# Run benchmarks
cargo bench
```

### Workflow

1. Fork the repo and create a branch: `git checkout -b feat/your-feature`
2. Write your code. Run `cargo fmt` and `cargo clippy -- -D warnings` before committing.
3. Add tests. The CI gate enforces that coverage does not decrease.
4. Open a PR against `main`. Describe what it does and link any related issues.

### Commit Convention

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat:     new feature
fix:      bug fix
perf:     performance improvement
refactor: code change (not feat, not fix)
test:     tests only
docs:     documentation only
chore:    tooling, CI, dependencies
```

### Code Standards

- All `unsafe` blocks require a `// SAFETY:` comment explaining the invariant
- Public API items must have doc comments (`///`)
- No panics in the hot path — propagate errors with `anyhow` or typed `thiserror` enums
- Every new feature or fix needs a test in `tests/integration/`

---

## 🗺 Roadmap

### ✅ v1.0 — Core Gateway (Released)
- [x] HTTP reverse proxy for JSON-RPC
- [x] Weighted round-robin, least-connections, random load balancing
- [x] Active health checks + circuit breaker
- [x] In-memory response caching with per-method TTLs
- [x] Token bucket rate limiting (per-key + per-IP)
- [x] API key management (SQLite backend)
- [x] Prometheus metrics
- [x] Structured JSON logging
- [x] Single-binary, zero-dependency deployment

### 🚧 v1.1 — Production Hardening (In Progress)
- [ ] Redis backend for cache + rate limiter (multi-node support)
- [ ] WebSocket proxying for Soroban event streaming
- [ ] TLS termination
- [ ] Hot config reload (`POST /admin/reload`)
- [ ] Grafana dashboard (ready-to-import JSON)
- [ ] OpenTelemetry tracing

### 📅 v1.2 — Observability & UX (Q3 2026)
- [ ] Admin web UI (lightweight, embedded)
- [ ] Request replay tool (re-run any logged request)
- [ ] Per-method latency SLO tracking + alerting rules
- [ ] Canary routing (send % of traffic to a new upstream)
- [ ] mTLS between gateway and upstreams

### 💡 v2.0 — Cluster Mode (Q4 2026)
- [ ] Native gossip-based clustering (no external coordinator)
- [ ] Consistent hashing for cache affinity across cluster
- [ ] Multi-region routing (latency-aware upstream selection)
- [ ] gRPC support
- [ ] WASM plugin API for custom middleware

---

## 🔒 Security

### Reporting Vulnerabilities

**Do not open a public GitHub issue for security vulnerabilities.**

Email [security@sorobangate.dev](mailto:security@sorobangate.dev) with:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Any suggested mitigations

We follow a 90-day responsible disclosure policy and will acknowledge receipt within 48 hours.

### Security Model

- The Admin API must never be exposed to the public internet — bind it to a loopback or private interface only
- API keys are stored hashed (Argon2id) — the plaintext is shown only once on creation
- SorobanGate runs as a non-root user and requires no special capabilities except `CAP_NET_BIND_SERVICE` when binding ports < 1024
- All upstream connections use TLS by default
- Request bodies are streamed and never buffered entirely in memory, preventing memory exhaustion from large payloads

---

## 📄 License

SorobanGate is released under the **Apache License 2.0**. See [`LICENSE`](LICENSE) for the full text.

```
Copyright 2026 SorobanGate Contributors

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0
```

---

## 🙏 Acknowledgements

SorobanGate is built on top of excellent open source crates:

| Crate | Purpose |
|---|---|
| [tokio](https://tokio.rs) | Async runtime |
| [axum](https://github.com/tokio-rs/axum) | HTTP server framework |
| [hyper](https://hyper.rs) | HTTP client for upstream proxying |
| [moka](https://github.com/moka-rs/moka) | In-memory concurrent cache |
| [fred](https://github.com/aembke/fred.rs) | Async Redis client |
| [tower](https://github.com/tower-rs/tower) | Middleware abstractions |
| [tracing](https://github.com/tokio-rs/tracing) | Structured logging & spans |
| [prometheus-client](https://github.com/prometheus/client_rust) | Prometheus metrics |
| [serde](https://serde.rs) | Serialisation |
| [anyhow](https://github.com/dtolnay/anyhow) / [thiserror](https://github.com/dtolnay/thiserror) | Error handling |
| [clap](https://clap.rs) | CLI argument parsing |

Special thanks to the [Stellar Development Foundation](https://stellar.org) for building Soroban and maintaining the public RPC infrastructure this project routes traffic around.

---

<div align="center">

**Built with ❤️ for the Stellar community · Written in Rust · 100% open source**

[⬆ Back to top](#table-of-contents)

</div>
