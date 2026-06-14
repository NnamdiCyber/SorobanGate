use axum::{
    Router, routing::{get, post, delete},
    extract::State,
    http::StatusCode,
    Json,
};
use serde_json::json;

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

async fn keys_list_handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "error": "Not Implemented",
            "message": "GET /admin/keys is not yet implemented"
        })),
    )
}

async fn keys_create_handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "error": "Not Implemented",
            "message": "POST /admin/keys is not yet implemented"
        })),
    )
}

async fn keys_delete_handler(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "error": "Not Implemented",
            "message": format!("DELETE /admin/keys/{} is not yet implemented", id)
        })),
    )
}
