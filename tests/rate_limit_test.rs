mod common;

use serde_json::json;

#[tokio::test]
async fn test_rate_limit_allows_burst_then_rejects() {
    let resp_val = json!({ "jsonrpc": "2.0", "id": 1, "result": "ok" });
    let (mock_addr, _shutdown) = common::start_mock(resp_val).await;
    // ip_fallback_rps = 2, burst = 2
    let gw_addr = common::start_gateway_with_config(
        &format!("http://{}", mock_addr),
        true,
        false,
    )
    .await;

    let client = reqwest::Client::new();
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "getHealth", "params": [] });
    let url = format!("http://{}", gw_addr);

    // First burst (2) should succeed
    let r1 = client.post(&url).json(&body).send().await.unwrap();
    let r2 = client.post(&url).json(&body).send().await.unwrap();
    assert_eq!(r1.status(), 200, "burst request 1 should succeed");
    assert_eq!(r2.status(), 200, "burst request 2 should succeed");

    // Third immediate request should be rate-limited
    let r3 = client.post(&url).json(&body).send().await.unwrap();
    assert_eq!(r3.status(), 429, "request 3 should be rate limited");
}
