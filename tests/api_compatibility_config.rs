//! Integration tests for compatibility configuration CRUD (Story 4.1).
//!
//! Tests that mutate the shared global config row are marked `#[serial]`
//! to prevent race conditions across parallel test execution.

mod common;

use kora::api::compatibility::COMPATIBILITY_LEVELS;
use reqwest::Client;
use serial_test::serial;

// -- Global compatibility --

#[tokio::test]
#[serial]
async fn get_global_compatibility_returns_backward_default() {
    let base = common::spawn_server().await;
    let client = Client::new();

    let resp = common::api::get_global_compatibility(&client, &base).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["compatibilityLevel"], "BACKWARD");
}

#[tokio::test]
#[serial]
async fn set_global_compatibility_updates_level() {
    let base = common::spawn_server().await;
    let client = Client::new();

    let resp = common::api::set_global_compatibility(&client, &base, "FULL").await;
    assert_eq!(resp.status(), 200);

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
    assert_eq!(resp.status(), 422);

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
        assert_eq!(resp.status(), 200, "should accept level {level}");

        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["compatibility"], *level);
    }

    // Restore default
    common::api::set_global_compatibility(&client, &base, "BACKWARD").await;
}

// -- Per-subject compatibility --

#[tokio::test]
#[serial]
async fn get_subject_compatibility_falls_back_to_global() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("compat-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = common::api::get_subject_compatibility(&client, &base, &subject).await;
    assert_eq!(resp.status(), 200);

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
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["compatibility"], "NONE");

    // Verify via GET
    let resp = common::api::get_subject_compatibility(&client, &base, &subject).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["compatibilityLevel"], "NONE");
}

#[tokio::test]
#[serial]
async fn delete_subject_compatibility_falls_back_to_global() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("compat-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    // Set per-subject config
    common::api::set_subject_compatibility(&client, &base, &subject, "NONE").await;

    // Delete per-subject config
    let resp = common::api::delete_subject_compatibility(&client, &base, &subject).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["compatibility"], "BACKWARD");

    // Verify GET now returns global default
    let resp = common::api::get_subject_compatibility(&client, &base, &subject).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["compatibilityLevel"], "BACKWARD");
}

#[tokio::test]
async fn subject_compatibility_returns_404_for_unknown_subject() {
    let base = common::spawn_server().await;
    let client = Client::new();

    let resp = common::api::get_subject_compatibility(&client, &base, "nonexistent").await;
    assert_eq!(resp.status(), 404);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40401);

    let resp = common::api::set_subject_compatibility(&client, &base, "nonexistent", "FULL").await;
    assert_eq!(resp.status(), 404);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40401);

    let resp = common::api::delete_subject_compatibility(&client, &base, "nonexistent").await;
    assert_eq!(resp.status(), 404);
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
    assert_eq!(resp.status(), 422);

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
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["compatibilityLevel"], "NONE");

    // Restore global default
    common::api::set_global_compatibility(&client, &base, "BACKWARD").await;
}
