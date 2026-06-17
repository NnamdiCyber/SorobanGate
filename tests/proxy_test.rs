mod common;

use serde_json::json;

#[tokio::test]
async fn test_proxy_forwards_request_to_upstream() {
    let expected = json!({ "jsonrpc": "2.0", "id": 1, "result": "ok" });
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

#[tokio::test]
async fn test_health_endpoint_returns_ok() {
    let expected = json!({ "jsonrpc": "2.0", "id": 1, "result": "ok" });
    let (mock_addr, _shutdown) = common::start_mock(expected).await;
    let gw_addr = common::start_gateway(&format!("http://{}", mock_addr)).await;

    let resp = reqwest::Client::new()
        .get(format!("http://{}/health", gw_addr))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_invalid_json_returns_400() {
    let expected = json!({ "jsonrpc": "2.0", "id": 1, "result": "ok" });
    let (mock_addr, _shutdown) = common::start_mock(expected).await;
    let gw_addr = common::start_gateway(&format!("http://{}", mock_addr)).await;

    let resp = reqwest::Client::new()
        .post(format!("http://{}", gw_addr))
        .header("Content-Type", "application/json")
        .body("not valid json")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}
