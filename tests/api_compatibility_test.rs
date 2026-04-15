//! Integration tests for compatibility test endpoints
//! (POST /compatibility/subjects/{subject}/versions/{version},
//!  POST /compatibility/subjects/{subject}/versions).

mod common;

use reqwest::StatusCode;

// -- Avro backward compatibility --

#[tokio::test]
async fn compat_backward_compatible_returns_true() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("compat-bw-ok-{}", uuid::Uuid::new_v4());

    // V1 has [id:int], V2 adds [name:optional] with default — backward compatible.
    common::api::register_schema(&client, &base, &subject, common::COMPAT_AVRO_V1).await;

    let resp = client
        .post(format!(
            "{base}/compatibility/subjects/{subject}/versions/latest"
        ))
        .json(&serde_json::json!({"schema": common::COMPAT_AVRO_V2}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["is_compatible"], true);
}

#[tokio::test]
async fn compat_backward_incompatible_returns_false() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("compat-bw-fail-{}", uuid::Uuid::new_v4());

    // Register V1, test common::COMPAT_AVRO_INCOMPAT which adds required "email" — backward incompatible.
    common::api::register_schema(&client, &base, &subject, common::COMPAT_AVRO_V1).await;

    let resp = client
        .post(format!(
            "{base}/compatibility/subjects/{subject}/versions/latest"
        ))
        .json(&serde_json::json!({"schema": common::COMPAT_AVRO_INCOMPAT}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["is_compatible"], false);
}

// -- Avro forward compatibility --

#[tokio::test]
async fn compat_forward_compatible_returns_true() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("compat-fw-ok-{}", uuid::Uuid::new_v4());

    // Register V2 (has optional "name"). Test V1 (no "name") in FORWARD mode.
    // FORWARD: old (V2) reads new (V1) data. V2 can read V1 — "name" has default → compatible.
    common::api::register_schema(&client, &base, &subject, common::COMPAT_AVRO_V2).await;

    client
        .put(format!("{base}/config/{subject}"))
        .json(&serde_json::json!({"compatibility": "FORWARD"}))
        .send()
        .await
        .unwrap();

    let resp = client
        .post(format!(
            "{base}/compatibility/subjects/{subject}/versions/latest"
        ))
        .json(&serde_json::json!({"schema": common::COMPAT_AVRO_V1}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["is_compatible"], true);
}

#[tokio::test]
async fn compat_forward_incompatible_returns_false() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("compat-fw-fail-{}", uuid::Uuid::new_v4());

    // Register V1, test common::COMPAT_AVRO_INCOMPAT in FORWARD mode.
    // FORWARD: old (V1) reads new (INCOMPAT) data. V1 can't read INCOMPAT data (unknown "email") — wait,
    // actually Avro readers ignore unknown fields. The real issue is the other direction.
    // FORWARD means writer=new, reader=old. can_read(new, old).
    // V1 as reader reads INCOMPAT data: V1 doesn't have "email", it ignores it. V1 has "id", INCOMPAT has "id". Compatible!
    // For forward-incompatible: register common::COMPAT_AVRO_INCOMPAT (with "email" required), test V1 against it.
    // FORWARD: old(INCOMPAT) reads new(V1). INCOMPAT as reader requires "email" but V1 data has no "email" → incompatible.
    common::api::register_schema(&client, &base, &subject, common::COMPAT_AVRO_INCOMPAT).await;

    client
        .put(format!("{base}/config/{subject}"))
        .json(&serde_json::json!({"compatibility": "FORWARD"}))
        .send()
        .await
        .unwrap();

    let resp = client
        .post(format!(
            "{base}/compatibility/subjects/{subject}/versions/latest"
        ))
        .json(&serde_json::json!({"schema": common::COMPAT_AVRO_V1}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["is_compatible"], false);
}

// -- FULL and NONE modes --

#[tokio::test]
async fn compat_full_requires_both_directions() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("compat-full-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::COMPAT_AVRO_V1).await;

    client
        .put(format!("{base}/config/{subject}"))
        .json(&serde_json::json!({"compatibility": "FULL"}))
        .send()
        .await
        .unwrap();

    // common::COMPAT_AVRO_INCOMPAT adds required "email" — backward-incompatible AND forward-incompatible.
    let resp = client
        .post(format!(
            "{base}/compatibility/subjects/{subject}/versions/latest"
        ))
        .json(&serde_json::json!({"schema": common::COMPAT_AVRO_INCOMPAT}))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        body["is_compatible"], false,
        "FULL mode should fail for incompatible schema"
    );
}

#[tokio::test]
async fn compat_none_always_compatible() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("compat-none-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::COMPAT_AVRO_V1).await;

    client
        .put(format!("{base}/config/{subject}"))
        .json(&serde_json::json!({"compatibility": "NONE"}))
        .send()
        .await
        .unwrap();

    let totally_different =
        r#"{"type":"record","name":"Different","fields":[{"name":"x","type":"string"}]}"#;
    let resp = client
        .post(format!(
            "{base}/compatibility/subjects/{subject}/versions/latest"
        ))
        .json(&serde_json::json!({"schema": totally_different}))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["is_compatible"], true);
}

// -- Verbose mode --

#[tokio::test]
async fn compat_verbose_returns_messages() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("compat-verbose-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::COMPAT_AVRO_V1).await;

    let resp = client
        .post(format!(
            "{base}/compatibility/subjects/{subject}/versions/latest?verbose=true"
        ))
        .json(&serde_json::json!({"schema": common::COMPAT_AVRO_INCOMPAT}))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["is_compatible"], false);
    assert!(
        body["messages"].is_array(),
        "verbose should include messages array"
    );
    assert!(
        !body["messages"].as_array().unwrap().is_empty(),
        "messages should not be empty for incompatible schema"
    );
}

// -- Test against specific version number --

#[tokio::test]
async fn compat_test_against_specific_version() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("compat-specver-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::COMPAT_AVRO_V1).await;
    common::api::register_schema(&client, &base, &subject, common::COMPAT_AVRO_V2).await;

    // Test V3 against version 1 specifically (not "latest").
    let resp = client
        .post(format!(
            "{base}/compatibility/subjects/{subject}/versions/1"
        ))
        .json(&serde_json::json!({"schema": common::COMPAT_AVRO_V3}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["is_compatible"], true);
}

// -- Test against all versions --

#[tokio::test]
async fn compat_test_against_all_versions_incompatible() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("compat-all-fail-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::COMPAT_AVRO_V1).await;
    common::api::register_schema(&client, &base, &subject, common::COMPAT_AVRO_V2).await;

    // COMPAT_AVRO_INCOMPAT adds required "email" — incompatible with both V1 and V2.
    let resp = client
        .post(format!("{base}/compatibility/subjects/{subject}/versions"))
        .json(&serde_json::json!({"schema": common::COMPAT_AVRO_INCOMPAT}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["is_compatible"], false);
}

#[tokio::test]
async fn compat_test_against_all_versions() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("compat-all-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::COMPAT_AVRO_V1).await;
    common::api::register_schema(&client, &base, &subject, common::COMPAT_AVRO_V2).await;

    // V3 adds another optional field — backward compatible with both V1 and V2.
    let resp = client
        .post(format!("{base}/compatibility/subjects/{subject}/versions"))
        .json(&serde_json::json!({"schema": common::COMPAT_AVRO_V3}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["is_compatible"], true);
}

// -- Test against "latest" --

#[tokio::test]
async fn compat_test_against_latest() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("compat-latest-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::COMPAT_AVRO_V1).await;
    common::api::register_schema(&client, &base, &subject, common::COMPAT_AVRO_V2).await;

    // Test V3 against "latest" (which is V2).
    let resp = client
        .post(format!(
            "{base}/compatibility/subjects/{subject}/versions/latest"
        ))
        .json(&serde_json::json!({"schema": common::COMPAT_AVRO_V3}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["is_compatible"], true);
}

// -- Error codes --

#[tokio::test]
async fn compat_nonexistent_subject_returns_40401() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!(
            "{base}/compatibility/subjects/nonexistent/versions/latest"
        ))
        .json(&serde_json::json!({"schema": common::COMPAT_AVRO_V1}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40401);
}

#[tokio::test]
async fn compat_nonexistent_version_returns_40402() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("compat-nover-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::COMPAT_AVRO_V1).await;

    let resp = client
        .post(format!(
            "{base}/compatibility/subjects/{subject}/versions/99"
        ))
        .json(&serde_json::json!({"schema": common::COMPAT_AVRO_V2}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40402);
}

#[tokio::test]
async fn compat_invalid_schema_returns_42201() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("compat-invalid-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::COMPAT_AVRO_V1).await;

    let resp = client
        .post(format!(
            "{base}/compatibility/subjects/{subject}/versions/latest"
        ))
        .json(&serde_json::json!({"schema": "not valid json"}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42201);
}

// JSON Schema and Protobuf diff engine coverage is in:
// - tests/confluent_json_schema_compat.rs (251 Confluent test cases)
// - tests/confluent_protobuf_compat.rs (43 Confluent test cases)
// - tests/confluent_circular_ref.rs (circular $ref tests)
