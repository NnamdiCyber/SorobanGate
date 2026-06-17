use std::net::SocketAddr;
use std::sync::Arc;
use axum::Router;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// Starts a mock JSON-RPC server returning the given response for every request.
/// Returns (socket_addr, shutdown_sender).
pub async fn start_mock_rpc(response: serde_json::Value) -> (SocketAddr, oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        let resp = Arc::new(response);
        let app = Router::new().fallback({
            let resp = resp.clone();
            move || {
                let resp = resp.clone();
                async move { axum::Json((*resp).clone()) }
            }
        });
        axum::serve(listener, app)
            .with_graceful_shutdown(async { rx.await.ok(); })
            .await
            .ok();
    });

    (addr, tx)
}

/// Starts a mock server that returns 503 Service Unavailable for all requests.
pub async fn start_failing_rpc() -> (SocketAddr, oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        let app = Router::new().fallback(|| async {
            (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                axum::Json(json!({ "error": "simulated failure" })),
            )
        });
        axum::serve(listener, app)
            .with_graceful_shutdown(async { rx.await.ok(); })
            .await
            .ok();
    });

    (addr, tx)
}
