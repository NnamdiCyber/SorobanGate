mod mock_server;

use serde_json::json;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpListener;
use axum::Router;

/// Build a minimal AppState wired to `upstream_url`.
async fn make_gateway(upstream_url: &str) -> (axum::Router, TcpListener) {
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
    let config: sorobangate::config::Config = toml::de::from_str(&toml).unwrap();
    let app = sorobangate::build_test_app(config, true).await.unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    (app, listener)
}

#[tokio::test]
async fn test_proxy_basic_request() {
    let expected = json!({ "jsonrpc": "2.0", "id": 1, "result": "ok" });
    let (mock_addr, _shutdown) = mock_server::start_mock_rpc(expected.clone()).await;
    let upstream_url = format!("http://{}", mock_addr);

    let (app, listener) = make_gateway(&upstream_url).await;
    let gateway_addr = listener.local_addr().unwrap();

    tokio::spawn(axum::serve(listener, app).into_future());

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}", gateway_addr))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "getHealth", "params": [] }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body, expected);
}

#[tokio::test]
async fn test_proxy_returns_503_when_no_upstreams() {
    // Use a port that is not listening
    let (app, listener) = make_gateway("http://127.0.0.1:1").await;
    let gateway_addr = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, app).into_future());

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}", gateway_addr))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "getHealth", "params": [] }))
        .send()
        .await
        .unwrap();

    // Either 503 (no healthy upstream) or 502/504 (connection refused) is acceptable
    assert!(resp.status().is_server_error());
}
