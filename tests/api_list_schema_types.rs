//! Integration test for listing supported schema types (GET /schemas/types).

mod common;

use reqwest::StatusCode;

#[tokio::test]
async fn list_schema_types() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{base}/schemas/types"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body: Vec<String> = resp.json().await.unwrap();
    assert_eq!(body, vec!["AVRO", "JSON", "PROTOBUF"]);
}
