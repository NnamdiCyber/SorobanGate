use axum::Router;
use std::future::IntoFuture;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// Start a mock JSON-RPC server returning the given value for all requests.
pub async fn start_mock(response: serde_json::Value) -> (SocketAddr, oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let app = Router::new().fallback(move || {
            let r = response.clone();
            async move { axum::Json(r) }
        });
        axum::serve(listener, app)
            .with_graceful_shutdown(async { rx.await.ok(); })
            .await
            .ok();
    });
    (addr, tx)
}

/// Build and serve a gateway on a free port, returning its address.
pub async fn start_gateway(upstream_url: &str) -> SocketAddr {
    start_gateway_with_config(upstream_url, false, false).await
}

pub async fn start_gateway_with_config(
    upstream_url: &str,
    rate_limit: bool,
    cache: bool,
) -> SocketAddr {
    let cache_section = if cache {
        r#"
[cache]
enabled = true
max_memory_mb = 64

[[cache.rules]]
methods = ["getLatestLedger"]
ttl_secs = 60
"#
    } else {
        "[cache]\nenabled = false\n"
    };

    let rate_section = if rate_limit {
        r#"
[rate_limit]
enabled = true
store = "memory"
ip_fallback_rps = 2
ip_fallback_burst = 2
"#
    } else {
        "[rate_limit]\nenabled = false\n"
    };

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

[auth]
enabled = false

[telemetry]
metrics_enabled = false

{}{}
"#,
        upstream_url, rate_section, cache_section
    );
    let config: sorobangate::config::Config = toml::de::from_str(&toml).unwrap();
    let app = sorobangate::server::build_test_app(config, true).await.unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, app).into_future());
    addr
}
