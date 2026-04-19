//! Integration tests for compatibility configuration CRUD (Story 4.1).
//!
//! Tests that mutate the shared global config row are marked `#[serial]`
//! to prevent race conditions across parallel test execution.

mod common;

use kora::api::compatibility::COMPATIBILITY_LEVELS;
use reqwest::{Client, StatusCode};
use serial_test::serial;

// -- Global compatibility --

#[tokio::test]
#[serial]
async fn get_global_compatibility_returns_backward_default() {
    let base = common::spawn_server().await;
    let client = Client::new();

    let resp = common::api::get_global_compatibility(&client, &base).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["compatibilityLevel"], "BACKWARD");
}

#[tokio::test]
#[serial]
async fn set_global_compatibility_updates_level() {
    let base = common::spawn_server().await;
    let client = Client::new();

    let resp = common::api::set_global_compatibility(&client, &base, "FULL").await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["compatibility"], "FULL");

    // Verify via GET
    let resp = common::api::get_global_compatibility(&client, &base).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["compatibilityLevel"], "FULL");

    // Restore default
    common::api::set_global_compatibility(&client, &base, "BACKWARD").await;
}

#[tokio::test]
async fn set_global_compatibility_rejects_invalid_level() {
    let base = common::spawn_server().await;
    let client = Client::new();

    let resp = common::api::set_global_compatibility(&client, &base, "INVALID").await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42203);
}

#[tokio::test]
#[serial]
async fn set_global_compatibility_accepts_all_valid_levels() {
    let base = common::spawn_server().await;
    let client = Client::new();

    for level in COMPATIBILITY_LEVELS {
        let resp = common::api::set_global_compatibility(&client, &base, level).await;
        assert_eq!(resp.status(), StatusCode::OK, "should accept level {level}");

        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["compatibility"], *level);
    }

    // Restore default
    common::api::set_global_compatibility(&client, &base, "BACKWARD").await;
}

#[tokio::test]
#[serial]
async fn get_global_compatibility_accepts_default_to_global_param() {
    let base = common::spawn_server().await;
    let client = Client::new();

    let resp = client
        .get(format!("{base}/config?defaultToGlobal=true"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["compatibilityLevel"], "BACKWARD");
}

#[tokio::test]
#[serial]
async fn delete_global_compatibility_resets_to_backward() {
    let base = common::spawn_server().await;
    let client = Client::new();

    common::api::set_global_compatibility(&client, &base, "FULL").await;

    let resp = client
        .delete(format!("{base}/config"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = common::api::get_global_compatibility(&client, &base).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["compatibilityLevel"], "BACKWARD");
}

// -- Per-subject compatibility --

#[tokio::test]
#[serial]
async fn get_subject_compatibility_without_config_returns_40408() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("compat-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    // Without defaultToGlobal → 40408.
    let resp = common::api::get_subject_compatibility(&client, &base, &subject).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40408);

    // With defaultToGlobal=true → falls back to global.
    let resp = client
        .get(format!("{base}/config/{subject}?defaultToGlobal=true"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["compatibilityLevel"], "BACKWARD");
}

#[tokio::test]
async fn set_subject_compatibility_sets_override() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("compat-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = common::api::set_subject_compatibility(&client, &base, &subject, "NONE").await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["compatibility"], "NONE");

    // Verify via GET
    let resp = common::api::get_subject_compatibility(&client, &base, &subject).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["compatibilityLevel"], "NONE");
}

#[tokio::test]
#[serial]
async fn delete_subject_compatibility_returns_previous_level() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("compat-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    // Set per-subject config to NONE.
    common::api::set_subject_compatibility(&client, &base, &subject, "NONE").await;

    // Delete per-subject config → returns previous Config object.
    let resp = common::api::delete_subject_compatibility(&client, &base, &subject).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["compatibilityLevel"], "NONE");

    // Verify GET now returns 40408 (no per-subject config).
    let resp = common::api::get_subject_compatibility(&client, &base, &subject).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_subject_compatibility_without_config_returns_40401() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("compat-noconfig-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    // No per-subject config set → DELETE returns 40401 (Confluent: subjectNotFoundException).
    let resp = common::api::delete_subject_compatibility(&client, &base, &subject).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40401);
}

#[tokio::test]
async fn subject_compatibility_nonexistent_returns_40408_not_40401() {
    let base = common::spawn_server().await;
    let client = Client::new();

    // Confluent does not check subject existence — only config existence.
    // GET on nonexistent subject with no config → 40408.
    let resp = common::api::get_subject_compatibility(&client, &base, "nonexistent").await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40408);

    // PUT on nonexistent subject succeeds (Confluent allows config on any subject name).
    let resp = common::api::set_subject_compatibility(&client, &base, "nonexistent", "FULL").await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Clean up the config we just set.
    let resp = common::api::delete_subject_compatibility(&client, &base, "nonexistent").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["compatibilityLevel"], "FULL");

    // DELETE on nonexistent subject with no config → 40401 (Confluent: subjectNotFoundException).
    let resp = common::api::delete_subject_compatibility(&client, &base, "nonexistent").await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40401);
}

#[tokio::test]
async fn set_subject_compatibility_rejects_invalid_level() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("compat-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = common::api::set_subject_compatibility(&client, &base, &subject, "BOGUS").await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42203);
}

// -- Override priority --

#[tokio::test]
#[serial]
async fn get_subject_compatibility_returns_override_not_global() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("compat-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    // Set global to FULL, subject to NONE
    common::api::set_global_compatibility(&client, &base, "FULL").await;
    common::api::set_subject_compatibility(&client, &base, &subject, "NONE").await;

    // Subject should return its own override, not the global
    let resp = common::api::get_subject_compatibility(&client, &base, &subject).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["compatibilityLevel"], "NONE");

    // Restore global default
    common::api::set_global_compatibility(&client, &base, "BACKWARD").await;
}

// -- Python-style booleans (case-insensitive query params) --

#[tokio::test]
#[serial]
async fn get_global_compatibility_accepts_python_style_default_to_global() {
    let base = common::spawn_server().await;
    let client = Client::new();

    // Python's str(True) → "True"
    let resp = client
        .get(format!("{base}/config?defaultToGlobal=True"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn get_subject_compatibility_accepts_python_style_default_to_global() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("py-compat-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    // Python's str(True) → "True"
    let resp = client
        .get(format!("{base}/config/{subject}?defaultToGlobal=True"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["compatibilityLevel"], "BACKWARD");
}
