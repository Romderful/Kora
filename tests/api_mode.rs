//! Integration tests for registry mode CRUD and enforcement (Story 6.1).
//!
//! Tests that mutate the shared global mode row are marked `#[file_serial]`
//! to prevent race conditions across parallel test execution.

mod common;

use kora::api::mode::VALID_MODES;
use reqwest::{Client, StatusCode};
use serial_test::file_serial;

// -- Global mode --

#[tokio::test]
#[file_serial]
async fn get_global_mode_returns_readwrite_default() {
    let base = common::spawn_server().await;
    let client = Client::new();

    let resp = common::api::get_global_mode(&client, &base).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["mode"], "READWRITE");
}

#[tokio::test]
#[file_serial]
async fn set_global_mode_updates_mode() {
    let base = common::spawn_server().await;
    let client = Client::new();

    let resp = common::api::set_global_mode(&client, &base, "READONLY").await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["mode"], "READONLY");

    // Verify via GET
    let resp = common::api::get_global_mode(&client, &base).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["mode"], "READONLY");

    // Restore default
    common::api::set_global_mode(&client, &base, "READWRITE").await;
}

#[tokio::test]
async fn set_global_mode_rejects_invalid_mode() {
    let base = common::spawn_server().await;
    let client = Client::new();

    let resp = common::api::set_global_mode(&client, &base, "INVALID").await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42204);
}

#[tokio::test]
#[file_serial]
async fn set_global_mode_accepts_all_valid_modes() {
    let base = common::spawn_server().await;
    let client = Client::new();

    for mode in VALID_MODES {
        let resp = common::api::set_global_mode(&client, &base, mode).await;
        assert_eq!(resp.status(), StatusCode::OK, "should accept mode {mode}");

        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["mode"], *mode);
    }

    // Restore default
    common::api::set_global_mode(&client, &base, "READWRITE").await;
}

#[tokio::test]
#[file_serial]
async fn set_global_mode_with_force_param() {
    let base = common::spawn_server().await;
    let client = Client::new();

    let resp = client
        .put(format!("{base}/mode?force=true"))
        .json(&serde_json::json!({"mode": "READONLY"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["mode"], "READONLY");

    // Restore default
    common::api::set_global_mode(&client, &base, "READWRITE").await;
}

#[tokio::test]
#[file_serial]
async fn get_global_mode_accepts_default_to_global_param() {
    let base = common::spawn_server().await;
    let client = Client::new();

    let resp = client
        .get(format!("{base}/mode?defaultToGlobal=true"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["mode"], "READWRITE");
}

#[tokio::test]
#[file_serial]
async fn delete_global_mode_resets_to_readwrite() {
    let base = common::spawn_server().await;
    let client = Client::new();

    common::api::set_global_mode(&client, &base, "IMPORT").await;

    let resp = common::api::delete_global_mode(&client, &base).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Returns previous mode
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["mode"], "IMPORT");

    // Verify reset
    let resp = common::api::get_global_mode(&client, &base).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["mode"], "READWRITE");
}

#[tokio::test]
#[file_serial]
async fn delete_global_mode_when_already_default_returns_readwrite() {
    let base = common::spawn_server().await;
    let client = Client::new();

    // Global mode is already READWRITE — delete should still succeed.
    let resp = common::api::delete_global_mode(&client, &base).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["mode"], "READWRITE");
}

// -- Per-subject mode --

#[tokio::test]
async fn get_subject_mode_without_config_returns_40409() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("mode-{}", uuid::Uuid::new_v4());

    // Without defaultToGlobal → 40409.
    let resp = common::api::get_subject_mode(&client, &base, &subject).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40409);
}

#[tokio::test]
#[file_serial]
async fn get_subject_mode_with_default_to_global_falls_back() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("mode-{}", uuid::Uuid::new_v4());

    // With defaultToGlobal=true → falls back to global.
    let resp = client
        .get(format!("{base}/mode/{subject}?defaultToGlobal=true"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["mode"], "READWRITE");
}

#[tokio::test]
async fn set_subject_mode_sets_override() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("mode-{}", uuid::Uuid::new_v4());

    let resp = common::api::set_subject_mode(&client, &base, &subject, "READONLY").await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["mode"], "READONLY");

    // Verify via GET
    let resp = common::api::get_subject_mode(&client, &base, &subject).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["mode"], "READONLY");
}

#[tokio::test]
async fn set_subject_mode_rejects_invalid_mode() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("mode-{}", uuid::Uuid::new_v4());

    let resp = common::api::set_subject_mode(&client, &base, &subject, "BOGUS").await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42204);
}

#[tokio::test]
async fn delete_subject_mode_returns_previous_mode() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("mode-{}", uuid::Uuid::new_v4());

    // Set per-subject mode to IMPORT.
    common::api::set_subject_mode(&client, &base, &subject, "IMPORT").await;

    // Delete per-subject mode → returns previous mode.
    let resp = common::api::delete_subject_mode(&client, &base, &subject).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["mode"], "IMPORT");

    // Verify GET now returns 40409 (no per-subject config).
    let resp = common::api::get_subject_mode(&client, &base, &subject).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_subject_mode_without_config_returns_40401() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("mode-noconfig-{}", uuid::Uuid::new_v4());

    // No per-subject config set → DELETE returns 40401 (Confluent: SubjectNotFound).
    let resp = common::api::delete_subject_mode(&client, &base, &subject).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40401);
}

#[tokio::test]
#[file_serial]
async fn subject_mode_returns_override_not_global() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("mode-{}", uuid::Uuid::new_v4());

    // Set global to IMPORT, subject to READONLY
    common::api::set_global_mode(&client, &base, "IMPORT").await;
    common::api::set_subject_mode(&client, &base, &subject, "READONLY").await;

    // Subject should return its own override, not the global
    let resp = common::api::get_subject_mode(&client, &base, &subject).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["mode"], "READONLY");

    // Restore global default
    common::api::set_global_mode(&client, &base, "READWRITE").await;
}

// -- Cross-module isolation: mode + compat don't interfere --

#[tokio::test]
async fn set_subject_mode_does_not_create_phantom_compat_config() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("mode-phantom-{}", uuid::Uuid::new_v4());

    // Set mode only — no compat config should appear.
    common::api::set_subject_mode(&client, &base, &subject, "READONLY").await;

    // GET /config/{subject} should return 40408 (not configured), not 200.
    let resp = common::api::get_subject_compatibility(&client, &base, &subject).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40408);
}

#[tokio::test]
async fn delete_compat_config_preserves_mode_override() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("mode-compat-{}", uuid::Uuid::new_v4());

    // Set both mode and compat on same subject.
    common::api::set_subject_mode(&client, &base, &subject, "READONLY").await;
    common::api::set_subject_compatibility(&client, &base, &subject, "NONE").await;

    // Delete compat config.
    let resp = common::api::delete_subject_compatibility(&client, &base, &subject).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Mode override should still be present.
    let resp = common::api::get_subject_mode(&client, &base, &subject).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["mode"], "READONLY");
}

// -- Mode enforcement --

#[tokio::test]
#[file_serial]
async fn readonly_mode_blocks_schema_registration() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("mode-enforce-{}", uuid::Uuid::new_v4());

    // Set global mode to READONLY
    common::api::set_global_mode(&client, &base, "READONLY").await;

    // Attempt to register a schema → should be rejected
    let resp = client
        .post(format!("{base}/subjects/{subject}/versions"))
        .json(&serde_json::json!({"schema": common::AVRO_SCHEMA_V1}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42205);

    // Restore default
    common::api::set_global_mode(&client, &base, "READWRITE").await;
}

#[tokio::test]
#[file_serial]
async fn readonly_per_subject_blocks_registration_for_that_subject() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("mode-subreadonly-{}", uuid::Uuid::new_v4());

    // Set per-subject mode to READONLY (global stays READWRITE).
    common::api::set_subject_mode(&client, &base, &subject, "READONLY").await;

    // Registration on this subject should be blocked.
    let resp = client
        .post(format!("{base}/subjects/{subject}/versions"))
        .json(&serde_json::json!({"schema": common::AVRO_SCHEMA_V1}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42205);

    // Other subjects should still work (global is READWRITE).
    let other = format!("mode-subreadonly-other-{}", uuid::Uuid::new_v4());
    let resp = client
        .post(format!("{base}/subjects/{other}/versions"))
        .json(&serde_json::json!({"schema": common::AVRO_SCHEMA_V1}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
#[file_serial]
async fn readonly_mode_blocks_subject_deletion() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("mode-del-{}", uuid::Uuid::new_v4());

    // Register a schema first (while READWRITE).
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    // Set global mode to READONLY.
    common::api::set_global_mode(&client, &base, "READONLY").await;

    // Attempt to soft-delete → should be rejected.
    let resp = client
        .delete(format!("{base}/subjects/{subject}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42205);

    // Restore default
    common::api::set_global_mode(&client, &base, "READWRITE").await;
}

#[tokio::test]
#[file_serial]
async fn readonly_mode_blocks_version_deletion() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("mode-verdel-{}", uuid::Uuid::new_v4());

    // Register a schema.
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    // Set global mode to READONLY.
    common::api::set_global_mode(&client, &base, "READONLY").await;

    // Attempt to soft-delete version 1 → should be rejected.
    let resp = client
        .delete(format!("{base}/subjects/{subject}/versions/1"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42205);

    // Restore default
    common::api::set_global_mode(&client, &base, "READWRITE").await;
}

#[tokio::test]
#[file_serial]
async fn readwrite_mode_allows_schema_registration() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("mode-enforce-{}", uuid::Uuid::new_v4());

    // READWRITE (default) should allow registration
    let resp = client
        .post(format!("{base}/subjects/{subject}/versions"))
        .json(&serde_json::json!({"schema": common::AVRO_SCHEMA_V1}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
#[file_serial]
async fn readonly_override_per_subject_allows_registration_despite_global_readonly() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("mode-override-{}", uuid::Uuid::new_v4());

    // Set global mode to READONLY
    common::api::set_global_mode(&client, &base, "READONLY").await;

    // Set per-subject mode to READONLY_OVERRIDE
    common::api::set_subject_mode(&client, &base, &subject, "READONLY_OVERRIDE").await;

    // Registration should succeed despite global READONLY
    let resp = client
        .post(format!("{base}/subjects/{subject}/versions"))
        .json(&serde_json::json!({"schema": common::AVRO_SCHEMA_V1}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // But a different subject without override should still be blocked
    let other = format!("mode-blocked-{}", uuid::Uuid::new_v4());
    let resp = client
        .post(format!("{base}/subjects/{other}/versions"))
        .json(&serde_json::json!({"schema": common::AVRO_SCHEMA_V1}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42205);

    // Restore default
    common::api::set_global_mode(&client, &base, "READWRITE").await;
}

#[tokio::test]
#[file_serial]
async fn import_mode_allows_schema_registration() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("mode-import-{}", uuid::Uuid::new_v4());

    // Set global mode to IMPORT
    common::api::set_global_mode(&client, &base, "IMPORT").await;

    // IMPORT mode should allow registration
    let resp = client
        .post(format!("{base}/subjects/{subject}/versions"))
        .json(&serde_json::json!({"schema": common::AVRO_SCHEMA_V1}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Restore default
    common::api::set_global_mode(&client, &base, "READWRITE").await;
}

#[tokio::test]
#[file_serial]
async fn forward_mode_allows_schema_registration() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("mode-forward-{}", uuid::Uuid::new_v4());

    // Set global mode to FORWARD
    common::api::set_global_mode(&client, &base, "FORWARD").await;

    // FORWARD mode should allow registration (like IMPORT)
    let resp = client
        .post(format!("{base}/subjects/{subject}/versions"))
        .json(&serde_json::json!({"schema": common::AVRO_SCHEMA_V1}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Restore default
    common::api::set_global_mode(&client, &base, "READWRITE").await;
}

// -- Recursive delete --

#[tokio::test]
async fn delete_subject_mode_recursive_clears_child_subjects() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let parent = format!("mode-recur-{}", uuid::Uuid::new_v4());
    let child1 = format!("{parent}-child1");
    let child2 = format!("{parent}-child2");

    // Set modes on parent and children.
    common::api::set_subject_mode(&client, &base, &parent, "READONLY").await;
    common::api::set_subject_mode(&client, &base, &child1, "IMPORT").await;
    common::api::set_subject_mode(&client, &base, &child2, "READONLY_OVERRIDE").await;

    // Delete parent mode with recursive=true.
    let resp = client
        .delete(format!("{base}/mode/{parent}?recursive=true"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Parent mode is gone.
    let resp = common::api::get_subject_mode(&client, &base, &parent).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // Children modes are gone too.
    let resp = common::api::get_subject_mode(&client, &base, &child1).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let resp = common::api::get_subject_mode(&client, &base, &child2).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// -- Python-style booleans (case-insensitive query params) --

#[tokio::test]
#[file_serial]
async fn set_global_mode_accepts_python_style_force_true() {
    let base = common::spawn_server().await;
    let client = Client::new();

    // Python's str(True) → "True"
    let resp = client
        .put(format!("{base}/mode?force=True"))
        .json(&serde_json::json!({"mode": "READONLY"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Restore default
    common::api::set_global_mode(&client, &base, "READWRITE").await;
}

#[tokio::test]
#[file_serial]
async fn get_global_mode_accepts_python_style_default_to_global() {
    let base = common::spawn_server().await;
    let client = Client::new();

    // Python's str(True) → "True"
    let resp = client
        .get(format!("{base}/mode?defaultToGlobal=True"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["mode"], "READWRITE");
}

#[tokio::test]
async fn get_subject_mode_accepts_python_style_default_to_global() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let subject = format!("py-mode-{}", uuid::Uuid::new_v4());

    // Python's str(True) → "True"
    let resp = client
        .get(format!("{base}/mode/{subject}?defaultToGlobal=True"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["mode"], "READWRITE");
}

#[tokio::test]
async fn delete_subject_mode_accepts_python_style_recursive_true() {
    let base = common::spawn_server().await;
    let client = Client::new();
    let parent = format!("py-recur-{}", uuid::Uuid::new_v4());
    let child = format!("{parent}-child");

    common::api::set_subject_mode(&client, &base, &parent, "READONLY").await;
    common::api::set_subject_mode(&client, &base, &child, "IMPORT").await;

    // Python's str(True) → "True"
    let resp = client
        .delete(format!("{base}/mode/{parent}?recursive=True"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = common::api::get_subject_mode(&client, &base, &child).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
