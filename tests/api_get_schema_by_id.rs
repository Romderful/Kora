//! Integration tests for schema retrieval by global ID (GET /schemas/ids/{id}).

mod common;

use reqwest::StatusCode;

#[tokio::test]
async fn get_schema_by_id_returns_schema() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("get-by-id-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = common::api::get_schema_by_id(&client, &base, id).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["schema"].as_str().unwrap(), common::AVRO_SCHEMA_V1);
    assert_eq!(body["schemaType"], "AVRO");
    assert_eq!(body["id"], id);
}

#[tokio::test]
async fn get_schema_by_nonexistent_id_returns_404() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = common::api::get_schema_by_id(&client, &base, i64::MAX).await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40403);
}

#[tokio::test]
async fn get_soft_deleted_schema_by_id_still_returns_200() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("soft-del-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::delete_version(&client, &base, &subject, "1").await;

    let resp = common::api::get_schema_by_id(&client, &base, id).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["schema"].as_str().unwrap(), common::AVRO_SCHEMA_V1);
}

#[tokio::test]
async fn get_schema_by_invalid_id_returns_400() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    // Invalid ID (non-numeric) — axum rejects with 400.
    let resp = client
        .get(format!("{base}/schemas/ids/abc"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
