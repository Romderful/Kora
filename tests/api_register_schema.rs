//! Integration tests for schema registration (POST /subjects/{subject}/versions).

mod common;

use reqwest::StatusCode;
use sqlx::Row;

// -- Common (format-agnostic) --

#[tokio::test]
async fn register_schema_without_type_defaults_to_avro() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("default-type-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    assert!(id > 0);
}

#[tokio::test]
async fn register_schema_creates_subject_implicitly() {
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

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let count: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM subjects WHERE name = $1")
        .bind(&subject)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn register_schema_empty_subject_returns_422() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    // Null char subject — must be inline (edge case subject).
    let resp = client
        .post(format!("{base}/subjects/%00/versions"))
        .json(&serde_json::json!({"schema": common::AVRO_SCHEMA_V1}))
        .send()
        .await
        .unwrap();

    assert_ne!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn register_schema_missing_body_returns_422() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    // Empty body — must be inline (custom body/header).
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
async fn register_schema_lowercase_type_succeeds() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    // Custom schemaType field — must be inline (custom body).
    let resp = client
        .post(format!("{base}/subjects/lowercase-type/versions"))
        .json(&serde_json::json!({"schema": common::AVRO_SCHEMA_V1, "schemaType": "avro"}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn register_with_normalize_deduplicates_whitespace_variants() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("norm-reg-{}", uuid::Uuid::new_v4());

    let schema_compact = r#"{"type":"record","name":"Norm","fields":[{"name":"id","type":"int"}]}"#;
    let schema_spaced = r#"{  "type" : "record",  "name" : "Norm",  "fields" : [ { "name" : "id", "type" : "int" } ] }"#;

    let resp1 = client
        .post(format!("{base}/subjects/{subject}/versions?normalize=true"))
        .json(&serde_json::json!({"schema": schema_compact}))
        .send()
        .await
        .unwrap();
    let id1 = resp1.json::<serde_json::Value>().await.unwrap()["id"].as_i64().unwrap();

    let resp2 = client
        .post(format!("{base}/subjects/{subject}/versions?normalize=true"))
        .json(&serde_json::json!({"schema": schema_spaced}))
        .send()
        .await
        .unwrap();
    let id2 = resp2.json::<serde_json::Value>().await.unwrap()["id"].as_i64().unwrap();

    assert_eq!(id1, id2, "normalize=true should deduplicate schemas that differ only in whitespace");
}

#[tokio::test]
async fn register_with_subject_config_normalize_deduplicates() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("cfg-norm-{}", uuid::Uuid::new_v4());

    // Set subject-level normalize=true via config endpoint.
    client
        .put(format!("{base}/config/{subject}"))
        .json(&serde_json::json!({"compatibility": "NONE", "normalize": true}))
        .send()
        .await
        .unwrap();

    let schema_compact = r#"{"type":"record","name":"CfgNorm","fields":[{"name":"id","type":"int"}]}"#;
    let schema_spaced = r#"{  "type" : "record",  "name" : "CfgNorm",  "fields" : [ { "name" : "id", "type" : "int" } ] }"#;

    // Register WITHOUT ?normalize=true — config should drive normalization.
    let id1 = common::api::register_schema(&client, &base, &subject, schema_compact).await;
    let id2 = common::api::register_schema(&client, &base, &subject, schema_spaced).await;

    assert_eq!(id1, id2, "subject config normalize=true should deduplicate without query param");
}

// -- Avro Schema --

#[tokio::test]
async fn register_avro_schema_valid_succeeds() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let pool = common::pool().await;
    let subject = format!("valid-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
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
    assert_eq!(row.get::<String, _>("schema_text"), common::AVRO_SCHEMA_V1);

    let expected = kora::schema::parse(kora::schema::SchemaFormat::Avro, common::AVRO_SCHEMA_V1).unwrap();
    assert_eq!(row.get::<Option<String>, _>("canonical_form").as_deref(), Some(expected.canonical_form.as_str()));
    assert_eq!(row.get::<Option<String>, _>("fingerprint").as_deref(), Some(expected.fingerprint.as_str()));
}

#[tokio::test]
async fn register_avro_schema_idempotent_returns_same_id() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let pool = common::pool().await;
    let subject = format!("idempotent-{}", uuid::Uuid::new_v4());

    let id1 = common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    let id2 = common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    assert_eq!(id1, id2, "same schema should return same id");

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
async fn register_avro_schema_invalid_returns_422() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    // Invalid schema body — must be inline (custom body).
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

// -- JSON Schema --

#[tokio::test]
async fn register_json_schema_valid_succeeds() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("json-reg-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema_with_type(
        &client, &base, &subject, common::JSON_SCHEMA_V1, "JSON",
    ).await;
    assert!(id > 0);
}

#[tokio::test]
async fn register_json_schema_invalid_returns_422() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base}/subjects/json-bad/versions"))
        .json(&serde_json::json!({"schema": "not valid json", "schemaType": "JSON"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42201);
}

#[tokio::test]
async fn register_json_schema_retrieve_includes_type() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("json-type-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema_with_type(
        &client, &base, &subject, common::JSON_SCHEMA_V1, "JSON",
    ).await;

    let resp = common::api::get_schema_by_id(&client, &base, id).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["schemaType"], "JSON");
}

#[tokio::test]
async fn register_json_schema_idempotent_returns_same_id() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("json-idem-{}", uuid::Uuid::new_v4());

    let id1 = common::api::register_schema_with_type(
        &client, &base, &subject, common::JSON_SCHEMA_V1, "JSON",
    ).await;
    let id2 = common::api::register_schema_with_type(
        &client, &base, &subject, common::JSON_SCHEMA_V1, "JSON",
    ).await;
    assert_eq!(id1, id2);
}

#[tokio::test]
async fn register_json_schema_listed_under_versions() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("json-ver-{}", uuid::Uuid::new_v4());

    common::api::register_schema_with_type(
        &client, &base, &subject, common::JSON_SCHEMA_V1, "JSON",
    ).await;

    let versions = common::api::list_versions(&client, &base, &subject, common::ACTIVE_ONLY).await;
    assert_eq!(versions, vec![1]);
}

#[tokio::test]
async fn register_json_schema_reordered_keys_deduplicates_with_normalize() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("json-dedup-{}", uuid::Uuid::new_v4());

    let schema_a = r#"{"type":"object","properties":{"name":{"type":"string"}}}"#;
    let schema_b = r#"{"properties":{"name":{"type":"string"}},"type":"object"}"#;

    let resp1 = client
        .post(format!("{base}/subjects/{subject}/versions?normalize=true"))
        .json(&serde_json::json!({"schema": schema_a, "schemaType": "JSON"}))
        .send().await.unwrap();
    let id1 = resp1.json::<serde_json::Value>().await.unwrap()["id"].as_i64().unwrap();

    let resp2 = client
        .post(format!("{base}/subjects/{subject}/versions?normalize=true"))
        .json(&serde_json::json!({"schema": schema_b, "schemaType": "JSON"}))
        .send().await.unwrap();
    let id2 = resp2.json::<serde_json::Value>().await.unwrap()["id"].as_i64().unwrap();

    assert_eq!(id1, id2, "normalize=true should deduplicate reordered JSON keys");
}

#[tokio::test]
async fn register_json_schema_reordered_keys_creates_new_version_without_normalize() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("json-no-norm-{}", uuid::Uuid::new_v4());

    let schema_a = r#"{"type":"object","properties":{"name":{"type":"string"}}}"#;
    let schema_b = r#"{"properties":{"name":{"type":"string"}},"type":"object"}"#;

    let id1 = common::api::register_schema_with_type(&client, &base, &subject, schema_a, "JSON").await;
    let id2 = common::api::register_schema_with_type(&client, &base, &subject, schema_b, "JSON").await;

    assert_ne!(id1, id2, "without normalize, different raw text should create separate versions");
}

// -- Protobuf schema --

#[tokio::test]
async fn register_protobuf_schema_valid_succeeds() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("proto-reg-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema_with_type(
        &client, &base, &subject, common::PROTO_SCHEMA_V1, "PROTOBUF",
    ).await;
    assert!(id > 0);
}

#[tokio::test]
async fn register_protobuf_schema_invalid_returns_422() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base}/subjects/proto-bad/versions"))
        .json(&serde_json::json!({"schema": "not a proto file {{{", "schemaType": "PROTOBUF"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42201);
}

#[tokio::test]
async fn register_protobuf_schema_retrieve_includes_type() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("proto-type-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema_with_type(
        &client, &base, &subject, common::PROTO_SCHEMA_V1, "PROTOBUF",
    ).await;

    let resp = common::api::get_schema_by_id(&client, &base, id).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["schemaType"], "PROTOBUF");
}

#[tokio::test]
async fn register_protobuf_schema_idempotent_returns_same_id() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("proto-idem-{}", uuid::Uuid::new_v4());

    let id1 = common::api::register_schema_with_type(
        &client, &base, &subject, common::PROTO_SCHEMA_V1, "PROTOBUF",
    ).await;
    let id2 = common::api::register_schema_with_type(
        &client, &base, &subject, common::PROTO_SCHEMA_V1, "PROTOBUF",
    ).await;
    assert_eq!(id1, id2);
}

#[tokio::test]
async fn register_protobuf_schema_listed_under_versions() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("proto-ver-{}", uuid::Uuid::new_v4());

    common::api::register_schema_with_type(
        &client, &base, &subject, common::PROTO_SCHEMA_V1, "PROTOBUF",
    ).await;

    let versions = common::api::list_versions(&client, &base, &subject, common::ACTIVE_ONLY).await;
    assert_eq!(versions, vec![1]);
}
