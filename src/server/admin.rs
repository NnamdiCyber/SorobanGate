use axum::{
    Router, routing::{get, post, delete},
    extract::State,
    http::StatusCode,
    Json,
};
use serde_json::json;

use crate::auth::KeyStore;
use crate::pool::{HealthStatus};
use crate::pool::circuit_breaker::CircuitBreakerState;

pub fn router() -> Router<crate::server::AppState> {
    Router::new()
        .route("/admin/status", get(status_handler))
        .route("/admin/reload", post(reload_handler))
        .route("/admin/cache", delete(cache_flush_handler))
        .route("/admin/cache/stats", get(cache_stats_handler))
        .route("/admin/upstreams/{url}/enable", post(upstream_enable_handler))
        .route("/admin/upstreams/{url}/disable", post(upstream_disable_handler))
        .route("/admin/keys", get(keys_list_handler))
        .route("/admin/keys", post(keys_create_handler))
        .route("/admin/keys/{id}", delete(keys_delete_handler))
}

async fn status_handler(
    State(state): State<crate::server::AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let uptime = state.start_time.elapsed().as_secs();

    let pools: Vec<serde_json::Value> = state.pools.iter().map(|pool| {
        let upstreams: Vec<serde_json::Value> = pool.upstreams.iter().map(|u| {
            let mstate = u.mutable.lock().unwrap();
            let cb_state = match mstate.circuit_breaker.state() {
                CircuitBreakerState::Closed => "closed",
                CircuitBreakerState::Open => "open",
                CircuitBreakerState::HalfOpen => "half-open",
            };
            let health = if mstate.health == HealthStatus::Healthy { "healthy" } else { "unhealthy" };
            let latency_ms = u.latency_us() / 1000;
            json!({
                "url": u.url,
                "state": health,
                "circuit_breaker": cb_state,
                "active_connections": u.active_connections(),
                "weight": u.weight,
                "latency_p99_ms": latency_ms,
            })
        }).collect();
        json!({
            "name": pool.name,
            "algorithm": format!("{:?}", pool.algorithm).to_lowercase(),
            "total_upstreams": pool.upstreams.len(),
            "healthy_upstreams": pool.healthy_upstreams().len(),
            "upstreams": upstreams,
        })
    }).collect();

    (StatusCode::OK, Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_secs": uptime,
        "pools": pools,
    })))
}

async fn reload_handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({ "error": "Hot reload not yet implemented" })),
    )
}

async fn cache_flush_handler(
    State(state): State<crate::server::AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.cache {
        Some(ref cache) => {
            cache.flush();
            tracing::info!("Cache flushed via admin API");
            (StatusCode::OK, Json(json!({ "status": "ok", "message": "Cache flushed" })))
        }
        None => (StatusCode::OK, Json(json!({ "status": "ok", "message": "Cache is disabled" }))),
    }
}

async fn cache_stats_handler(
    State(state): State<crate::server::AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.cache {
        Some(ref cache) => {
            let stats = cache.stats();
            let hit_rate = if stats.hit_count + stats.miss_count > 0 {
                stats.hit_count as f64 / (stats.hit_count + stats.miss_count) as f64
            } else {
                0.0
            };
            (StatusCode::OK, Json(json!({
                "hit_count": stats.hit_count,
                "miss_count": stats.miss_count,
                "hit_rate": (hit_rate * 10000.0).round() / 10000.0,
                "size": stats.size,
                "eviction_count": stats.eviction_count,
            })))
        }
        None => (StatusCode::OK, Json(json!({ "enabled": false }))),
    }
}

async fn upstream_enable_handler(
    State(state): State<crate::server::AppState>,
    axum::extract::Path(encoded_url): axum::extract::Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    set_upstream_health(&state, &encoded_url, true)
}

async fn upstream_disable_handler(
    State(state): State<crate::server::AppState>,
    axum::extract::Path(encoded_url): axum::extract::Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    set_upstream_health(&state, &encoded_url, false)
}

fn set_upstream_health(
    state: &crate::server::AppState,
    encoded_url: &str,
    enable: bool,
) -> (StatusCode, Json<serde_json::Value>) {
    // URL-decode the upstream url path param
    let target_url = percent_decode(encoded_url);

    for pool in &state.pools {
        for upstream in &pool.upstreams {
            if upstream.url == target_url {
                let mut mstate = upstream.mutable.lock().unwrap();
                if enable {
                    mstate.health = HealthStatus::Healthy;
                    mstate.circuit_breaker = crate::pool::circuit_breaker::CircuitBreaker::new(
                        5,
                        std::time::Duration::from_secs(30),
                    );
                    tracing::info!(upstream = %target_url, "Upstream force-enabled via admin API");
                } else {
                    mstate.health = HealthStatus::Unhealthy;
                    tracing::info!(upstream = %target_url, "Upstream force-disabled via admin API");
                }
                let action = if enable { "enabled" } else { "disabled" };
                return (StatusCode::OK, Json(json!({ "status": "ok", "upstream": target_url, "action": action })));
            }
        }
    }

    (StatusCode::NOT_FOUND, Json(json!({ "error": format!("Upstream '{}' not found", target_url) })))
}

fn percent_decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s)
        .decode_utf8()
        .map(|s| s.into_owned())
        .unwrap_or_else(|_| s.to_string())
}

async fn keys_list_handler(
    State(state): State<crate::server::AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.key_store {
        Some(ref store) => match store.list_keys() {
            Ok(keys) => {
                let safe_keys: Vec<serde_json::Value> = keys.iter().map(|k| json!({
                    "id": k.id,
                    "tier": k.tier,
                    "label": k.label,
                    "is_revoked": k.is_revoked,
                    "created_at": k.created_at,
                })).collect();
                let total = safe_keys.len();
                (StatusCode::OK, Json(json!({ "keys": safe_keys, "total": total })))
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to list API keys");
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "Failed to list API keys" })))
            }
        },
        None => (StatusCode::NOT_IMPLEMENTED, Json(json!({ "error": "Key store is disabled" }))),
    }
}

async fn keys_create_handler(
    State(state): State<crate::server::AppState>,
    Json(payload): Json<serde_json::Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    let raw_key = match payload.get("key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return (StatusCode::BAD_REQUEST, Json(json!({ "error": "Field 'key' is required" }))),
    };
    let tier = payload.get("tier").and_then(|v| v.as_str()).unwrap_or("default");
    let label = payload.get("label").and_then(|v| v.as_str()).unwrap_or("");

    match state.key_store {
        Some(ref store) => match store.create_key(&raw_key, tier, label) {
            Ok(entry) => {
                tracing::info!(key_id = %entry.id, tier = %entry.tier, "API key created");
                (StatusCode::CREATED, Json(json!({
                    "status": "ok",
                    "key": { "id": entry.id, "tier": entry.tier, "label": entry.label, "created_at": entry.created_at }
                })))
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to create API key");
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "Failed to create API key" })))
            }
        },
        None => (StatusCode::NOT_IMPLEMENTED, Json(json!({ "error": "Key store is disabled" }))),
    }
}

async fn keys_delete_handler(
    State(state): State<crate::server::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.key_store {
        Some(ref store) => match store.revoke_key(&id) {
            Ok(true) => {
                tracing::info!(key_id = %id, "API key revoked");
                (StatusCode::OK, Json(json!({ "status": "ok", "message": "Key revoked" })))
            }
            Ok(false) => (StatusCode::NOT_FOUND, Json(json!({ "error": format!("Key '{}' not found", id) }))),
            Err(e) => {
                tracing::error!(error = %e, "Failed to revoke API key");
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "Failed to revoke API key" })))
            }
        },
        None => (StatusCode::NOT_IMPLEMENTED, Json(json!({ "error": "Key store is disabled" }))),
    }
}
