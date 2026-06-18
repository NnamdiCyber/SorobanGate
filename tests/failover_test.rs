mod common;

use serde_json::json;

#[tokio::test]
async fn test_failover_when_upstream_unreachable() {
    // Point at a port that is not listening — should get a 5xx
    let gw_addr = common::start_gateway("http://127.0.0.1:1").await;

    let resp = reqwest::Client::new()
        .post(format!("http://{}", gw_addr))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "getHealth", "params": [] }))
        .send()
        .await
        .unwrap();

    // Expect 503 (no healthy upstreams) or 502/504 (connection error)
    assert!(
        resp.status().is_server_error(),
        "Expected 5xx, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_upstream_recovery_serves_requests() {
    // Start a working upstream, verify traffic flows
    let expected = json!({ "jsonrpc": "2.0", "id": 1, "result": "recovered" });
    let (mock_addr, _shutdown) = common::start_mock(expected.clone()).await;
    let gw_addr = common::start_gateway(&format!("http://{}", mock_addr)).await;

    let resp = reqwest::Client::new()
        .post(format!("http://{}", gw_addr))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "getHealth", "params": [] }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body, expected);
}
