//! Integration tests for schema registration (POST /subjects/{subject}/versions).

mod common;

use reqwest::StatusCode;
use sqlx::Row;

const VALID_AVRO: &str = r#"{"type":"record","name":"Test","fields":[{"name":"id","type":"int"}]}"#;

#[tokio::test]
async fn register_valid_avro_schema() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let pool = common::pool().await;

    let subject = format!("valid-{}", uuid::Uuid::new_v4());

    let resp = client
        .post(format!("{base}/subjects/{subject}/versions"))
        .json(&serde_json::json!({"schema": VALID_AVRO}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    let id = body["id"].as_i64().unwrap();
    assert!(id > 0);

    // Verify the row stored in DB.
    let row = sqlx::query(
        "SELECT version, schema_type, schema_text, canonical_form, fingerprint FROM schemas WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.get::<i32, _>("version"), 1);
    assert_eq!(row.get::<String, _>("schema_type"), "AVRO");
    assert_eq!(row.get::<String, _>("schema_text"), VALID_AVRO);

    let expected = kora::schema::parse(kora::schema::SchemaFormat::Avro, VALID_AVRO).unwrap();
    assert_eq!(row.get::<Option<String>, _>("canonical_form").as_deref(), Some(expected.canonical_form.as_str()));
    assert_eq!(row.get::<Option<String>, _>("fingerprint").as_deref(), Some(expected.fingerprint.as_str()));
}

#[tokio::test]
async fn register_same_schema_is_idempotent() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let pool = common::pool().await;

    let subject = format!("idempotent-{}", uuid::Uuid::new_v4());

    let resp1 = client
        .post(format!("{base}/subjects/{subject}/versions"))
        .json(&serde_json::json!({"schema": VALID_AVRO}))
        .send()
        .await
        .unwrap();
    let id1: serde_json::Value = resp1.json().await.unwrap();

    let resp2 = client
        .post(format!("{base}/subjects/{subject}/versions"))
        .json(&serde_json::json!({"schema": VALID_AVRO}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);
    let id2: serde_json::Value = resp2.json().await.unwrap();

    assert_eq!(id1["id"], id2["id"], "same schema should return same id");

    let count: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM schemas s JOIN subjects sub ON s.subject_id = sub.id WHERE sub.name = $1",
    )
    .bind(&subject)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "idempotent registration should not create duplicate rows");
}

#[tokio::test]
async fn register_invalid_schema_returns_422() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base}/subjects/bad-value/versions"))
        .json(&serde_json::json!({"schema": r#"{"not":"a schema"}"#}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42201);
}

#[tokio::test]
async fn register_without_schema_type_defaults_to_avro() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base}/subjects/default-type-value/versions"))
        .json(&serde_json::json!({"schema": VALID_AVRO}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["id"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn register_creates_subject_implicitly() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let pool = common::pool().await;

    let subject = format!("implicit-{}", uuid::Uuid::new_v4());

    let count: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM subjects WHERE name = $1")
        .bind(&subject)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);

    let resp = client
        .post(format!("{base}/subjects/{subject}/versions"))
        .json(&serde_json::json!({"schema": VALID_AVRO}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let count: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM subjects WHERE name = $1")
        .bind(&subject)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn register_with_empty_subject_returns_422() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base}/subjects/%00/versions"))
        .json(&serde_json::json!({"schema": VALID_AVRO}))
        .send()
        .await
        .unwrap();

    assert_ne!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn register_with_missing_body_returns_422() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base}/subjects/test-value/versions"))
        .header("content-type", "application/json")
        .body("{}")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42201, "should return Confluent error format");
}

#[tokio::test]
async fn register_with_lowercase_schema_type_succeeds() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base}/subjects/lowercase-type/versions"))
        .json(&serde_json::json!({"schema": VALID_AVRO, "schemaType": "avro"}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}
