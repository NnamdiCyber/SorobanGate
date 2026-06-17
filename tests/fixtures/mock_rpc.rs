/// Mock RPC server used in integration tests.
/// Starts a simple JSON-RPC HTTP server on a free port and returns (addr, shutdown_tx).
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

pub async fn start_mock_rpc(
    response: serde_json::Value,
) -> (SocketAddr, oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        let app = axum::Router::new()
            .fallback(move || {
                let resp = response.clone();
                async move { axum::Json(resp) }
            });
        axum::serve(listener, app)
            .with_graceful_shutdown(async { rx.await.ok(); })
            .await
            .ok();
    });

    (addr, tx)
}

/// Helper to build a minimal gateway config pointing at the given upstream URL.
pub fn test_config(upstream_url: &str) -> sorobangate::config::Config {
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
    toml::de::from_str(&toml).expect("test config parse failed")
}
