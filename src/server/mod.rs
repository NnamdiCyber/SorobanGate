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

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub start_time: Instant,
}

pub async fn serve(config: Config) -> anyhow::Result<()> {
    let state = AppState {
        config: Arc::new(config),
        start_time: Instant::now(),
    };

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
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "pools": state.config.pools.len(),
        "uptime_secs": uptime,
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
    // API key extraction stub
    next.run(request).await
}

async fn rate_limit_middleware(
    request: Request<axum::body::Body>,
    next: middleware::Next,
) -> impl IntoResponse {
    // Rate limiter check stub
    next.run(request).await
}

async fn cache_middleware(
    request: Request<axum::body::Body>,
    next: middleware::Next,
) -> impl IntoResponse {
    // Cache lookup stub
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
