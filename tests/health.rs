//! Integration tests for the health endpoint.

mod common;

use reqwest::StatusCode;

#[tokio::test]
async fn health_returns_200_when_pg_is_up() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{base}/health"))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(resp.status(), StatusCode::OK);

    let ct = resp
        .headers()
        .get("content-type")
        .expect("should have content-type")
        .to_str()
        .expect("content-type should be valid utf8");
    assert_eq!(ct, "application/vnd.schemaregistry.v1+json");

    let body: serde_json::Value = resp.json().await.expect("should parse json");
    assert_eq!(body["status"], "UP");
}
