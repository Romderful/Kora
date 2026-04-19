//! Integration tests for checking schema registration
//! (POST /subjects/{subject}).

mod common;

use reqwest::StatusCode;

#[tokio::test]
async fn check_schema_registered_returns_200() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("check-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = common::api::check_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["subject"], subject);
    assert_eq!(body["id"], id);
    assert_eq!(body["version"], 1);
    assert_eq!(body["schema"], common::AVRO_SCHEMA_V1);
    assert_eq!(body["schemaType"], "AVRO");
}

#[tokio::test]
async fn check_schema_unregistered_returns_40403() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("check-miss-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = common::api::check_schema(&client, &base, &subject, common::AVRO_SCHEMA_OTHER).await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40403);
}

#[tokio::test]
async fn check_schema_with_normalize_finds_match() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("norm-check-{}", uuid::Uuid::new_v4());

    let schema_compact =
        r#"{"type":"record","name":"NormCheck","fields":[{"name":"id","type":"int"}]}"#;
    let schema_spaced = r#"{  "type" : "record", "name" : "NormCheck",  "fields" : [ { "name" : "id", "type" : "int" } ] }"#;

    common::api::register_schema(&client, &base, &subject, schema_compact).await;

    let resp = client
        .post(format!("{base}/subjects/{subject}?normalize=true"))
        .json(&serde_json::json!({"schema": schema_spaced}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "normalize=true should find the match"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["subject"], subject);
    assert_eq!(body["version"], 1);
}

#[tokio::test]
async fn check_schema_without_normalize_misses_whitespace_variant() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("norm-miss-{}", uuid::Uuid::new_v4());

    let schema_compact =
        r#"{"type":"record","name":"NormMiss","fields":[{"name":"id","type":"int"}]}"#;
    let schema_spaced = r#"{  "type" : "record", "name" : "NormMiss",  "fields" : [ { "name" : "id", "type" : "int" } ] }"#;

    common::api::register_schema(&client, &base, &subject, schema_compact).await;

    // Without normalize → raw fingerprint mismatch → 40403.
    let resp = client
        .post(format!("{base}/subjects/{subject}"))
        .json(&serde_json::json!({"schema": schema_spaced}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40403);
}

#[tokio::test]
async fn check_schema_with_deleted_finds_soft_deleted() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("check-del-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::delete_subject(&client, &base, &subject).await;

    // Without deleted=true → 40401 (subject not found since soft-deleted).
    let resp = common::api::check_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // With deleted=true → finds the soft-deleted schema.
    let resp = client
        .post(format!("{base}/subjects/{subject}?deleted=true"))
        .json(&serde_json::json!({"schema": common::AVRO_SCHEMA_V1}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "deleted=true should find soft-deleted schema"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["subject"], subject);
    assert_eq!(body["id"], id);
    assert_eq!(body["version"], 1);
}

#[tokio::test]
async fn check_schema_nonexistent_subject_returns_40401() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp =
        common::api::check_schema(&client, &base, "nonexistent", common::AVRO_SCHEMA_V1).await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40401);
}

// -- Python-style booleans (case-insensitive query params) --

#[tokio::test]
async fn check_schema_with_python_style_normalize_true() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("py-norm-check-{}", uuid::Uuid::new_v4());

    let schema_compact =
        r#"{"type":"record","name":"PyCheck","fields":[{"name":"id","type":"int"}]}"#;
    let schema_spaced = r#"{  "type" : "record", "name" : "PyCheck",  "fields" : [ { "name" : "id", "type" : "int" } ] }"#;

    common::api::register_schema(&client, &base, &subject, schema_compact).await;

    // Python's str(True) → "True"
    let resp = client
        .post(format!("{base}/subjects/{subject}?normalize=True"))
        .json(&serde_json::json!({"schema": schema_spaced}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "normalize=True should find the match"
    );
}

#[tokio::test]
async fn check_schema_with_python_style_deleted_true() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("py-check-del-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::delete_subject(&client, &base, &subject).await;

    // Python's str(True) → "True"
    let resp = client
        .post(format!("{base}/subjects/{subject}?deleted=True"))
        .json(&serde_json::json!({"schema": common::AVRO_SCHEMA_V1}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "deleted=True should find soft-deleted schema"
    );
}
