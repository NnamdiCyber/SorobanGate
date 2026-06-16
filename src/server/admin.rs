use axum::{
    Router, routing::{get, post, delete},
    extract::State,
    http::StatusCode,
    Json,
};
use serde_json::json;

use crate::auth::KeyStore;

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

async fn status_handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "error": "Not Implemented",
            "message": "GET /admin/status is not yet implemented"
        })),
    )
}

async fn reload_handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "error": "Not Implemented",
            "message": "POST /admin/reload is not yet implemented"
        })),
    )
}

async fn cache_flush_handler(
    State(state): State<crate::server::AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.cache {
        Some(ref cache) => {
            cache.flush();
            tracing::info!("Cache flushed via admin API");
            (
                StatusCode::OK,
                Json(json!({ "status": "ok", "message": "Cache flushed" })),
            )
        }
        None => (
            StatusCode::OK,
            Json(json!({ "status": "ok", "message": "Cache is disabled, nothing to flush" })),
        ),
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
            (
                StatusCode::OK,
                Json(json!({
                    "hit_count": stats.hit_count,
                    "miss_count": stats.miss_count,
                    "hit_rate": (hit_rate * 10000.0).round() / 10000.0,
                    "size": stats.size,
                    "eviction_count": stats.eviction_count,
                })),
            )
        }
        None => (
            StatusCode::OK,
            Json(json!({
                "enabled": false,
                "message": "Cache is disabled"
            })),
        ),
    }
}

async fn upstream_enable_handler(
    axum::extract::Path(url): axum::extract::Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "error": "Not Implemented",
            "message": format!("POST /admin/upstreams/{}/enable is not yet implemented", url)
        })),
    )
}

async fn upstream_disable_handler(
    axum::extract::Path(url): axum::extract::Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "error": "Not Implemented",
            "message": format!("POST /admin/upstreams/{}/disable is not yet implemented", url)
        })),
    )
}

async fn keys_list_handler(
    State(state): State<crate::server::AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.key_store {
        Some(ref store) => {
            match store.list_keys() {
                Ok(keys) => {
                    let safe_keys: Vec<serde_json::Value> = keys.iter().map(|k| {
                        json!({
                            "id": k.id,
                            "tier": k.tier,
                            "label": k.label,
                            "is_revoked": k.is_revoked,
                            "created_at": k.created_at,
                        })
                    }).collect();
                    (StatusCode::OK, Json(json!({ "keys": safe_keys, "total": safe_keys.len() })))
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to list API keys");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": "Failed to list API keys" })),
                    )
                }
            }
        }
        None => (
            StatusCode::NOT_IMPLEMENTED,
            Json(json!({ "error": "Key store is disabled" })),
        ),
    }
}

async fn keys_create_handler(
    State(state): State<crate::server::AppState>,
    Json(payload): Json<serde_json::Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    let raw_key = payload.get("key").and_then(|v| v.as_str()).unwrap_or("");
    let tier = payload.get("tier").and_then(|v| v.as_str()).unwrap_or("default");
    let label = payload.get("label").and_then(|v| v.as_str()).unwrap_or("");

    if raw_key.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Field 'key' is required" })),
        );
    }

    match state.key_store {
        Some(ref store) => {
            match store.create_key(raw_key, tier, label) {
                Ok(entry) => {
                    tracing::info!(key_id = %entry.id, tier = %entry.tier, "API key created");
                    (
                        StatusCode::CREATED,
                        Json(json!({
                            "status": "ok",
                            "key": {
                                "id": entry.id,
                                "tier": entry.tier,
                                "label": entry.label,
                                "is_revoked": entry.is_revoked,
                                "created_at": entry.created_at,
                            }
                        })),
                    )
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to create API key");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": "Failed to create API key" })),
                    )
                }
            }
        }
        None => (
            StatusCode::NOT_IMPLEMENTED,
            Json(json!({ "error": "Key store is disabled" })),
        ),
    }
}

async fn keys_delete_handler(
    State(state): State<crate::server::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.key_store {
        Some(ref store) => {
            match store.revoke_key(&id) {
                Ok(true) => {
                    tracing::info!(key_id = %id, "API key revoked");
                    (StatusCode::OK, Json(json!({ "status": "ok", "message": "Key revoked" })))
                }
                Ok(false) => (
                    StatusCode::NOT_FOUND,
                    Json(json!({ "error": format!("Key '{}' not found", id) })),
                ),
                Err(e) => {
                    tracing::error!(error = %e, "Failed to revoke API key");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": "Failed to revoke API key" })),
                    )
                }
            }
        }
        None => (
            StatusCode::NOT_IMPLEMENTED,
            Json(json!({ "error": "Key store is disabled" })),
        ),
    }
}
