use axum::{
    http::StatusCode,
    response::IntoResponse,
    extract::State,
};

pub async fn proxy_handler(
    State(state): State<crate::server::AppState>,
    req: axum::http::Request<axum::body::Body>,
) -> impl IntoResponse {
    let method = req.method().clone();
    let uri = req.uri().clone();
    tracing::debug!(
        method = %method,
        uri = %uri,
        pools = state.config.pools.len(),
        "Proxy handler stub"
    );
    StatusCode::NOT_IMPLEMENTED
}
