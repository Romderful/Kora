//! Integration tests for schema retrieval by global ID (GET /schemas/ids/{id}).

mod common;

use reqwest::StatusCode;

const VALID_AVRO: &str = r#"{"type":"record","name":"Test","fields":[{"name":"id","type":"int"}]}"#;

#[tokio::test]
async fn get_schema_by_id_returns_schema() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let subject = format!("get-by-id-{}", uuid::Uuid::new_v4());

    // Register a schema first.
    let resp = client
        .post(format!("{base}/subjects/{subject}/versions"))
        .json(&serde_json::json!({"schema": VALID_AVRO}))
        .send()
        .await
        .unwrap();
    let id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();

    // Retrieve by global ID.
    let resp = client
        .get(format!("{base}/schemas/ids/{id}"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["schema"].as_str().unwrap(), VALID_AVRO);
}

#[tokio::test]
async fn get_schema_by_nonexistent_id_returns_404() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{base}/schemas/ids/{}", i64::MAX))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40403);
}

#[tokio::test]
async fn get_soft_deleted_schema_by_id_still_returns_200() {
    let base = common::spawn_server().await;
    let pool = common::pool().await;
    let client = reqwest::Client::new();

    let subject = format!("soft-del-{}", uuid::Uuid::new_v4());

    // Register a schema.
    let resp = client
        .post(format!("{base}/subjects/{subject}/versions"))
        .json(&serde_json::json!({"schema": VALID_AVRO}))
        .send()
        .await
        .unwrap();
    let id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();

    // Soft-delete it directly in the DB.
    sqlx::query("UPDATE schemas SET deleted = true WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();

    // GET should still return 200 — IDs are permanent.
    let resp = client
        .get(format!("{base}/schemas/ids/{id}"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["schema"].as_str().unwrap(), VALID_AVRO);
}

#[tokio::test]
async fn get_schema_by_invalid_id_returns_400() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{base}/schemas/ids/abc"))
        .send()
        .await
        .unwrap();

    // Axum rejects non-numeric path parameters with 400.
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
