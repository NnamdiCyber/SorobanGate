use axum::{
    extract::State,
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
use http_body_util::BodyExt;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

use crate::{cache::compute_cache_key, metrics, pool::UpstreamState, server::AppState};

pub async fn proxy_handler(
    State(state): State<AppState>,
    req: axum::http::Request<axum::body::Body>,
) -> Response {
    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::new_v4().to_string();

    let (_parts, body) = req.into_parts();
    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            tracing::warn!(request_id = %request_id, error = %e, "Failed to read request body");
            return json_error_response(StatusCode::BAD_REQUEST, -32700, "Parse error");
        }
    };

    let method = match crate::routing::extract_json_rpc_method(&body_bytes) {
        Some(m) => m,
        None => {
            tracing::warn!(request_id = %request_id, "Invalid JSON-RPC request");
            return json_error_response(StatusCode::BAD_REQUEST, -32700, "Parse error");
        }
    };

    let pool_name = state.router.route(&method).to_string();

    if let Some(ref cache) = state.cache {
        let ttl = state.cache_ttl_table.ttl_for(&method);
        if !ttl.is_zero() {
            let ck = compute_cache_key(&body_bytes);
            if let Some(cached_body) = cache.get(&ck) {
                let elapsed = start.elapsed();
                tracing::info!(
                    request_id = %request_id,
                    method = %method,
                    pool = %pool_name,
                    duration_ms = elapsed.as_millis() as u64,
                    status = 200,
                    cached = true,
                    "Cache hit"
                );
                if let Some(ref m) = state.metrics {
                    m.metrics
                        .cache_hits_total
                        .get_or_create(&metrics::MethodLabels {
                            method: method.clone(),
                        })
                        .inc();
                    m.metrics
                        .requests_total
                        .get_or_create(&metrics::RequestLabels {
                            method: method.clone(),
                            pool: pool_name.clone(),
                            status: "200".to_string(),
                            cached: "true".to_string(),
                        })
                        .inc();
                    m.metrics
                        .request_duration_seconds
                        .get_or_create(&metrics::MethodPoolLabels {
                            method: method.clone(),
                            pool: pool_name.clone(),
                        })
                        .observe(elapsed.as_secs_f64());
                }
                return (
                    StatusCode::OK,
                    [("Content-Type", "application/json")],
                    cached_body,
                )
                    .into_response();
            } else if let Some(ref m) = state.metrics {
                m.metrics
                    .cache_misses_total
                    .get_or_create(&metrics::MethodLabels {
                        method: method.clone(),
                    })
                    .inc();
            }
        }
    }

    let pool = match state.pools.iter().find(|p| p.name == pool_name) {
        Some(p) => p.clone(),
        None => {
            tracing::error!(
                request_id = %request_id,
                method = %method,
                pool = %pool_name,
                "Configured pool not found"
            );
            return json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                -32000,
                "Pool not configured",
            );
        }
    };

    let upstream = match pool.select_upstream() {
        Some(u) => u,
        None => {
            tracing::warn!(
                request_id = %request_id,
                method = %method,
                pool = %pool_name,
                "No healthy upstreams available"
            );
            return json_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                -32004,
                "No healthy upstreams available",
            );
        }
    };

    upstream.increment_connection_count();
    let result = forward_to_upstream(
        &upstream,
        &body_bytes,
        state.config.server.request_timeout_ms,
    )
    .await;
    upstream.decrement_connection_count();

    match result {
        Ok((upstream_status, upstream_body, upstream_headers)) => {
            let elapsed = start.elapsed();
            tracing::info!(
                request_id = %request_id,
                method = %method,
                pool = %pool_name,
                upstream = %upstream.url,
                duration_ms = elapsed.as_millis() as u64,
                status = %upstream_status.as_u16(),
                cached = false,
                "Proxy request completed"
            );

            if let Some(ref m) = state.metrics {
                m.metrics
                    .requests_total
                    .get_or_create(&metrics::RequestLabels {
                        method: method.clone(),
                        pool: pool_name.clone(),
                        status: upstream_status.as_u16().to_string(),
                        cached: "false".to_string(),
                    })
                    .inc();
                m.metrics
                    .request_duration_seconds
                    .get_or_create(&metrics::MethodPoolLabels {
                        method: method.clone(),
                        pool: pool_name.clone(),
                    })
                    .observe(elapsed.as_secs_f64());
                m.metrics
                    .upstream_latency_seconds
                    .get_or_create(&metrics::UpstreamLabels {
                        upstream: upstream.url.clone(),
                        pool: pool_name.clone(),
                    })
                    .observe(elapsed.as_secs_f64());
            }

            if let Some(ref cache) = state.cache {
                let ttl = state.cache_ttl_table.ttl_for(&method);
                if !ttl.is_zero() && upstream_status.is_success() {
                    let ck = compute_cache_key(&body_bytes);
                    cache.set(ck, upstream_body.clone(), ttl);
                }
            }

            let mut rb = Response::builder().status(upstream_status);
            for (name, value) in &upstream_headers {
                rb = rb.header(name, value);
            }
            rb.body(axum::body::Body::from(upstream_body))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        Err(e) => {
            {
                let mut state_mut = upstream.mutable.lock().unwrap();
                state_mut.circuit_breaker.record_failure();
            }

            tracing::error!(
                request_id = %request_id,
                method = %method,
                pool = %pool_name,
                upstream = %upstream.url,
                error = %e,
                "Upstream request failed"
            );

            let status = if e.to_string().contains("timed out") || e.to_string().contains("timeout")
            {
                StatusCode::GATEWAY_TIMEOUT
            } else {
                StatusCode::BAD_GATEWAY
            };

            if let Some(ref m) = state.metrics {
                m.metrics
                    .requests_total
                    .get_or_create(&metrics::RequestLabels {
                        method: method.clone(),
                        pool: pool_name.clone(),
                        status: status.as_u16().to_string(),
                        cached: "false".to_string(),
                    })
                    .inc();
            }

            json_error_response(status, -32000, &e.to_string())
        }
    }
}

async fn forward_to_upstream(
    upstream: &Arc<UpstreamState>,
    body: &[u8],
    timeout_ms: u64,
) -> Result<(StatusCode, Vec<u8>, HeaderMap), anyhow::Error> {
    let uri: Uri = upstream.url.parse()?;

    let req_body: http_body_util::Full<hyper::body::Bytes> =
        http_body_util::Full::from(hyper::body::Bytes::copy_from_slice(body));
    let req = hyper::Request::post(&uri)
        .header("Content-Type", "application/json")
        .body(req_body)?;

    let connector = hyper_util::client::legacy::connect::HttpConnector::new();
    let client = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build(connector);

    let timeout = Duration::from_millis(timeout_ms);
    let response = tokio::time::timeout(timeout, client.request(req))
        .await
        .map_err(|_| anyhow::anyhow!("Upstream request timed out after {}ms", timeout_ms))??;

    let status = response.status();
    let headers = response.headers().clone();
    let (_, resp_body) = response.into_parts();
    let collected = resp_body.collect().await?;
    let body_bytes = collected.to_bytes().to_vec();

    Ok((status, body_bytes, headers))
}

fn json_error_response(status: StatusCode, code: i32, message: &str) -> Response {
    (
        status,
        Json(json!({
            "jsonrpc": "2.0",
            "id": null,
            "error": {
                "code": code,
                "message": message,
            }
        })),
    )
        .into_response()
}
