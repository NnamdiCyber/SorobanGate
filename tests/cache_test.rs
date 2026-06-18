mod common;

use serde_json::json;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::test]
async fn test_cache_hit_does_not_re_query_upstream() {
    // Count how many times the upstream is hit
    let call_count = Arc::new(AtomicU32::new(0));
    let call_count_srv = call_count.clone();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mock_addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let app = axum::Router::new().fallback(move || {
            let c = call_count_srv.clone();
            async move {
                c.fetch_add(1, Ordering::Relaxed);
                axum::Json(json!({ "jsonrpc": "2.0", "id": 1, "result": { "ledger": 42 } }))
            }
        });
        axum::serve(listener, app).await.ok();
    });

    let gw_addr =
        common::start_gateway_with_config(&format!("http://{}", mock_addr), false, true).await;

    let client = reqwest::Client::new();
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "getLatestLedger", "params": [] });
    let url = format!("http://{}", gw_addr);

    // First request — cache miss, hits upstream
    let r1 = client.post(&url).json(&body).send().await.unwrap();
    assert_eq!(r1.status(), 200);

    // Second identical request — should be a cache hit, upstream NOT called again
    let r2 = client.post(&url).json(&body).send().await.unwrap();
    assert_eq!(r2.status(), 200);

    let hits = call_count.load(Ordering::Relaxed);
    assert_eq!(
        hits, 1,
        "upstream should only be called once due to caching, got {}",
        hits
    );
}
