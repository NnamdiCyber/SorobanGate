use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use serde_json::json;
use std::future::IntoFuture;
use tokio::net::TcpListener;
use tokio::runtime::Runtime;

fn make_config(upstream_url: &str) -> sorobangate::config::Config {
    let toml = format!(
        r#"
[server]
bind = "127.0.0.1:0"
admin_bind = "127.0.0.1:0"
metrics_bind = "127.0.0.1:0"
log_level = "error"

[[pools]]
name = "default"
algorithm = "wrr"

  [[pools.upstreams]]
  url = "{}"
  weight = 1

[routing]
default_pool = "default"

[rate_limit]
enabled = false

[auth]
enabled = false

[cache]
enabled = false

[telemetry]
metrics_enabled = false
"#,
        upstream_url
    );
    toml::de::from_str(&toml).unwrap()
}

fn bench_proxy_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    // Spin up a trivial mock upstream
    let mock_addr = rt.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let app = axum::Router::new().fallback(|| async {
                axum::Json(json!({ "jsonrpc": "2.0", "id": 1, "result": "ok" }))
            });
            axum::serve(listener, app).await.ok();
        });
        addr
    });

    let upstream_url = format!("http://{}", mock_addr);
    let config = make_config(&upstream_url);

    let gateway_addr = rt.block_on(async {
        let app = sorobangate::server::build_test_app(config, true)
            .await
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(axum::serve(listener, app).into_future());
        addr
    });

    let client = reqwest::blocking::Client::new();
    let url = format!("http://{}", gateway_addr);
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "getHealth", "params": [] });

    c.bench_with_input(
        BenchmarkId::new("proxy_request", "getHealth"),
        &(url, body),
        |b, (url, body)| {
            b.iter(|| {
                client
                    .post(url.as_str())
                    .json(body)
                    .send()
                    .expect("request failed");
            });
        },
    );
}

fn bench_cache_hit(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mock_addr = rt.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let app = axum::Router::new().fallback(|| async {
                axum::Json(json!({ "jsonrpc": "2.0", "id": 1, "result": { "ledger": 123 } }))
            });
            axum::serve(listener, app).await.ok();
        });
        addr
    });

    let toml = format!(
        r#"
[server]
bind = "127.0.0.1:0"
admin_bind = "127.0.0.1:0"
metrics_bind = "127.0.0.1:0"
log_level = "error"

[[pools]]
name = "default"
algorithm = "wrr"

  [[pools.upstreams]]
  url = "http://{}"
  weight = 1

[routing]
default_pool = "default"

[rate_limit]
enabled = false

[auth]
enabled = false

[cache]
enabled = true
max_memory_mb = 64

[[cache.rules]]
methods = ["getLatestLedger"]
ttl_secs = 60

[telemetry]
metrics_enabled = false
"#,
        mock_addr
    );
    let config: sorobangate::config::Config = toml::de::from_str(&toml).unwrap();

    let gateway_addr = rt.block_on(async {
        let app = sorobangate::server::build_test_app(config, true)
            .await
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(axum::serve(listener, app).into_future());
        addr
    });

    let client = reqwest::blocking::Client::new();
    let url = format!("http://{}", gateway_addr);
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "getLatestLedger", "params": [] });

    // Warm up cache
    client.post(&url).json(&body).send().ok();

    c.bench_with_input(
        BenchmarkId::new("proxy_request", "cache_hit"),
        &(url, body),
        |b, (url, body)| {
            b.iter(|| {
                client
                    .post(url.as_str())
                    .json(body)
                    .send()
                    .expect("request failed");
            });
        },
    );
}

criterion_group!(benches, bench_proxy_throughput, bench_cache_hit);
criterion_main!(benches);
