pub mod admin;
pub mod proxy;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    Router, routing::get,
    extract::State,
    http::{Request, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use tower_http::trace::TraceLayer;

use crate::{auth, cache, cache::Cache, config, config::Config, pool, rate_limit, rate_limit::RateLimiter, routing};
use crate::auth::KeyStore;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub start_time: Instant,
    pub pools: Vec<Arc<pool::UpstreamPool>>,
    pub router: routing::Router,
    pub cache: Option<Arc<dyn Cache + Send + Sync>>,
    pub cache_ttl_table: cache::TtlTable,
    pub rate_limiter: Option<Arc<rate_limit::RateLimiterDispatch>>,
    pub key_store: Option<Arc<auth::KeyStoreDispatch>>,
    pub auth_tiers: HashMap<String, config::KeyTier>,
}

pub async fn serve(mut config: Config, skip_initial_health_check: bool) -> anyhow::Result<()> {
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

    let key_store: Option<Arc<auth::KeyStoreDispatch>> = if config.auth.enabled {
        let db_path = config.auth.db_path.to_str().unwrap_or("sorobangate.db").to_string();
        Some(Arc::new(auth::KeyStoreDispatch::Sqlite(
            Arc::new(auth::store::sqlite::SqliteKeyStore::new(&db_path)?),
        )))
    } else {
        None
    };

    let auth_tiers_list = std::mem::take(&mut config.auth.tiers);
    let config = Arc::new(config);
    let auth_tiers: HashMap<String, config::KeyTier> = auth_tiers_list.into_iter()
        .map(|t| (t.name.clone(), t))
        .collect();

    let rate_limiter: Option<Arc<rate_limit::RateLimiterDispatch>> = if config.rate_limit.enabled {
        Some(Arc::new(rate_limit::RateLimiterDispatch::Memory(
            Arc::new(rate_limit::token_bucket::TokenBucketRateLimiter::new()),
        )))
    } else {
        None
    };

    let state = AppState {
        config: config.clone(),
        start_time: Instant::now(),
        pools: pools.clone(),
        router,
        cache: cache_backend,
        cache_ttl_table,
        rate_limiter,
        key_store,
        auth_tiers,
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
        .layer(middleware::from_fn_with_state(state.clone(), cache_middleware))
        .layer(middleware::from_fn_with_state(state.clone(), rate_limit_middleware))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(middleware::from_fn_with_state(state.clone(), gateway_middleware))
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
        rate_limit = state.rate_limiter.is_some(),
        auth = state.key_store.is_some(),
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
    State(_state): State<AppState>,
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
    State(state): State<AppState>,
    mut request: Request<axum::body::Body>,
    next: middleware::Next,
) -> impl IntoResponse {
    let config = &state.config;

    if !config.auth.enabled || state.key_store.is_none() {
        return next.run(request).await;
    }

    let api_key = auth::extract_api_key(request.headers(), request.uri());

    match api_key {
        Some(ref key) => {
            let store = state.key_store.as_ref().unwrap();
            match store.lookup(key) {
                Ok(Some(entry)) => {
                    if entry.is_revoked {
                        return auth_error_response(
                            StatusCode::FORBIDDEN,
                            -32003,
                            "API key has been revoked",
                        );
                    }
                    request.extensions_mut().insert(auth::AuthTier(entry.tier.clone()));
                    request.extensions_mut().insert(auth::AuthKeyId(entry.id.clone()));
                    tracing::debug!(tier = %entry.tier, key_id = %entry.id, "Authenticated request");
                    next.run(request).await
                }
                Ok(None) => {
                    tracing::warn!("Unknown API key presented");
                    if config.auth.allow_unauthenticated {
                        next.run(request).await
                    } else {
                        auth_error_response(
                            StatusCode::UNAUTHORIZED,
                            -32001,
                            "Invalid API key",
                        )
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Key store lookup failed");
                    auth_error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        -32002,
                        "Authentication service unavailable",
                    )
                }
            }
        }
        None => {
            if config.auth.allow_unauthenticated {
                next.run(request).await
            } else {
                auth_error_response(
                    StatusCode::UNAUTHORIZED,
                    -32001,
                    "Authentication required - provide API key via Authorization: Bearer, X-API-Key header, or api_key query parameter",
                )
            }
        }
    }
}

async fn rate_limit_middleware(
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: middleware::Next,
) -> impl IntoResponse {
    let config = &state.config;

    if !config.rate_limit.enabled || state.rate_limiter.is_none() {
        return next.run(request).await;
    }

    let limiter = state.rate_limiter.as_ref().unwrap();

    let (rate_key, rps, burst) = if let Some(tier_name) = request.extensions().get::<auth::AuthTier>() {
        let tier = state.auth_tiers.get(&tier_name.0);
        match tier {
            Some(t) => (format!("tier:{}", t.name), t.requests_per_second, t.burst),
            None => {
                let client_ip = extract_client_ip(request.headers(), request.uri());
                (format!("ip:{}", client_ip), config.rate_limit.ip_fallback_rps, config.rate_limit.ip_fallback_burst)
            }
        }
    } else {
        let client_ip = extract_client_ip(request.headers(), request.uri());
        (format!("ip:{}", client_ip), config.rate_limit.ip_fallback_rps, config.rate_limit.ip_fallback_burst)
    };

    match limiter.check_rate(&rate_key, rps, burst) {
        Ok(()) => next.run(request).await,
        Err(_) => {
            tracing::warn!(rate_key = %rate_key, rps = rps, burst = burst, "Rate limit exceeded");
            rate_limit_response()
        }
    }
}

async fn cache_middleware(
    State(_state): State<AppState>,
    request: Request<axum::body::Body>,
    next: middleware::Next,
) -> impl IntoResponse {
    next.run(request).await
}

fn extract_client_ip(headers: &axum::http::HeaderMap, _uri: &axum::http::Uri) -> String {
    if let Some(val) = headers.get("X-Forwarded-For") {
        if let Ok(s) = val.to_str() {
            if let Some(ip) = s.split(',').next() {
                let ip = ip.trim();
                if !ip.is_empty() {
                    return ip.to_string();
                }
            }
        }
    }
    if let Some(val) = headers.get("X-Real-IP") {
        if let Ok(s) = val.to_str() {
            if !s.is_empty() {
                return s.to_string();
            }
        }
    }
    "unknown".to_string()
}

fn rate_limit_response() -> Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        Json(json!({
            "jsonrpc": "2.0",
            "id": null,
            "error": {
                "code": -32005,
                "message": "Rate limit exceeded"
            }
        })),
    )
        .into_response()
}

fn auth_error_response(status: StatusCode, code: i32, message: impl Into<String>) -> Response {
    (
        status,
        Json(json!({
            "jsonrpc": "2.0",
            "id": null,
            "error": {
                "code": code,
                "message": message.into()
            }
        })),
    )
        .into_response()
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
