pub mod admin;
pub mod proxy;

use std::sync::Arc;
use std::time::Instant;

use axum::{
    Router, routing::get,
    extract::State,
    http::Request,
    middleware,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use tower_http::trace::TraceLayer;

use crate::{cache, cache::Cache, config::Config, pool, routing};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub start_time: Instant,
    pub pools: Vec<Arc<pool::UpstreamPool>>,
    pub router: routing::Router,
    pub cache: Option<Arc<dyn Cache + Send + Sync>>,
    pub cache_ttl_table: cache::TtlTable,
}

pub async fn serve(config: Config, skip_initial_health_check: bool) -> anyhow::Result<()> {
    let pools: Vec<Arc<pool::UpstreamPool>> = pool::create_pools(&config);
    let router = routing::Router::new(&config.routing);

    let cache_backend: Option<Arc<dyn Cache + Send + Sync>> = if config.cache.enabled {
        Some(Arc::new(cache::memory::MemoryCache::new(
            config.cache.max_memory_mb,
        )))
    } else {
        None
    };
    let cache_ttl_table = cache::TtlTable::new(&config.cache.rules);

    let state = AppState {
        config: Arc::new(config),
        start_time: Instant::now(),
        pools: pools.clone(),
        router,
        cache: cache_backend,
        cache_ttl_table,
    };

    // Spawn health check task
    let health_pools = pools.clone();
    let health_config = state.config.clone();
    tokio::spawn(async move {
        pool::health::run_health_checks(health_pools, health_config, skip_initial_health_check).await;
    });

    let app = Router::new()
        .route("/health", get(health_handler))
        .fallback(proxy::proxy_handler)
        .layer(middleware::from_fn(cache_middleware))
        .layer(middleware::from_fn(rate_limit_middleware))
        .layer(middleware::from_fn(auth_middleware))
        .layer(middleware::from_fn(gateway_middleware))
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());

    let admin_app = admin::router()
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());

    let main_listener = tokio::net::TcpListener::bind(state.config.server.bind).await?;
    let admin_listener = tokio::net::TcpListener::bind(state.config.server.admin_bind).await?;

    tracing::info!(
        http = %state.config.server.bind,
        admin = %state.config.server.admin_bind,
        pools = pools.len(),
        "Server listening"
    );

    let shutdown = Arc::new(tokio::sync::Notify::new());

    let main_handle: tokio::task::JoinHandle<anyhow::Result<()>> = {
        let shutdown = shutdown.clone();
        let config = state.config.clone();
        let app = app.clone();
        tokio::spawn(async move {
            if config.tls.enabled {
                let tls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(
                    &config.tls.cert_file,
                    &config.tls.key_file,
                )
                .await?;

                let handle = axum_server::Handle::new();
                let shutdown_handle = handle.clone();
                tokio::spawn(async move {
                    shutdown.notified().await;
                    shutdown_handle.graceful_shutdown(Some(std::time::Duration::from_secs(10)));
                });

                axum_server::bind_rustls(config.server.bind, tls_config)
                    .handle(handle)
                    .serve(app.into_make_service())
                    .await
                    .map_err(|e| anyhow::anyhow!("Main server (TLS) error: {}", e))
            } else {
                axum::serve(main_listener, app)
                    .with_graceful_shutdown(async move { shutdown.notified().await })
                    .await
                    .map_err(|e| anyhow::anyhow!("Main server error: {}", e))
            }
        })
    };


    let admin_handle: tokio::task::JoinHandle<anyhow::Result<()>> = {
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            axum::serve(admin_listener, admin_app)
                .with_graceful_shutdown(async move { shutdown.notified().await })
                .await
                .map_err(|e| anyhow::anyhow!("Admin server error: {}", e))
        })
    };

    shutdown_signal().await;

    tracing::info!("Shutdown signal received, starting graceful shutdown");
    shutdown.notify_waiters();

    main_handle.await??;
    admin_handle.await??;

    tracing::info!("Server stopped");

    Ok(())
}

async fn health_handler(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let uptime = state.start_time.elapsed().as_secs();

    let pools_status: Vec<serde_json::Value> = state.pools.iter().map(|pool| {
        let total = pool.upstreams.len();
        let healthy = pool.healthy_upstreams().len();
        json!({
            "name": pool.name,
            "total_upstreams": total,
            "healthy_upstreams": healthy,
            "algorithm": format!("{:?}", pool.algorithm),
            "upstreams": pool.upstreams.iter().map(|u| {
                let state = u.mutable.lock().unwrap();
                json!({
                    "url": u.url,
                    "weight": u.weight,
                    "health": format!("{:?}", state.health),
                    "circuit_breaker": format!("{:?}", state.circuit_breaker.state()),
                    "active_connections": u.active_connections(),
                    "latency_us": u.latency_us(),
                })
            }).collect::<Vec<_>>(),
        })
    }).collect();

    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_secs": uptime,
        "pools": pools_status,
    }))
}

async fn gateway_middleware(
    request: Request<axum::body::Body>,
    next: middleware::Next,
) -> impl IntoResponse {
    let method = request.method().clone();
    let uri = request.uri().clone();

    let response = next.run(request).await;

    let status = response.status();
    tracing::debug!(method = %method, uri = %uri, status = %status, "Request completed");

    response
}

async fn auth_middleware(
    request: Request<axum::body::Body>,
    next: middleware::Next,
) -> impl IntoResponse {
    next.run(request).await
}

async fn rate_limit_middleware(
    request: Request<axum::body::Body>,
    next: middleware::Next,
) -> impl IntoResponse {
    next.run(request).await
}

async fn cache_middleware(
    request: Request<axum::body::Body>,
    next: middleware::Next,
) -> impl IntoResponse {
    next.run(request).await
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        let mut signal = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate(),
        )
        .expect("failed to install SIGTERM handler");
        signal.recv().await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
