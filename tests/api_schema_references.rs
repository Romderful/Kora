//! Integration tests for schema references and dependency protection (Story 2.4).
//!
//! Tests reference validation on registration and deletion protection.

mod common;

use reqwest::StatusCode;

// -- Registration with valid references (AC #1) --

#[tokio::test]
async fn register_schema_with_valid_reference() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let ref_subject = format!("ref-base-{}", uuid::Uuid::new_v4());
    let dep_subject = format!("ref-dep-{}", uuid::Uuid::new_v4());

    // Register the referenced schema first.
    common::api::register_schema(&client, &base, &ref_subject, common::AVRO_SCHEMA_V1).await;

    // Register a schema that references it.
    let refs = serde_json::json!([
        {"name": "Base", "subject": ref_subject, "version": 1}
    ]);
    let resp = common::api::register_schema_with_refs(&client, &base, &dep_subject, common::AVRO_SCHEMA_V2, &refs).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["id"].as_i64().is_some());
}

#[tokio::test]
async fn register_schema_without_references_still_works() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("ref-norefs-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    assert!(id > 0);
}

// -- Registration with invalid references (AC #2) --

#[tokio::test]
async fn register_schema_with_nonexistent_subject_reference_returns_422() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("ref-badsubj-{}", uuid::Uuid::new_v4());

    let refs = serde_json::json!([
        {"name": "Missing", "subject": "nonexistent-subject", "version": 1}
    ]);
    let resp = common::api::register_schema_with_refs(&client, &base, &subject, common::AVRO_SCHEMA_V1, &refs).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42201);
}

#[tokio::test]
async fn register_schema_with_nonexistent_version_reference_returns_422() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let ref_subject = format!("ref-badver-{}", uuid::Uuid::new_v4());
    let dep_subject = format!("ref-badver-dep-{}", uuid::Uuid::new_v4());

    // Register v1 only.
    common::api::register_schema(&client, &base, &ref_subject, common::AVRO_SCHEMA_V1).await;

    // Reference v99 which doesn't exist.
    let refs = serde_json::json!([
        {"name": "Base", "subject": ref_subject, "version": 99}
    ]);
    let resp = common::api::register_schema_with_refs(&client, &base, &dep_subject, common::AVRO_SCHEMA_V2, &refs).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42201);
}

// -- Deletion protection (AC #3) --

#[tokio::test]
async fn hard_delete_referenced_version_returns_422() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let ref_subject = format!("ref-prot-{}", uuid::Uuid::new_v4());
    let dep_subject = format!("ref-prot-dep-{}", uuid::Uuid::new_v4());

    // Register base schema + dependent schema with reference.
    common::api::register_schema(&client, &base, &ref_subject, common::AVRO_SCHEMA_V1).await;
    let refs = serde_json::json!([
        {"name": "Base", "subject": ref_subject, "version": 1}
    ]);
    common::api::register_schema_with_refs(&client, &base, &dep_subject, common::AVRO_SCHEMA_V2, &refs).await;

    // Soft-delete the referenced version first (required before hard-delete).
    common::api::delete_version(&client, &base, &ref_subject, "1").await;

    // Hard-delete should be blocked.
    let resp = common::api::hard_delete_version(&client, &base, &ref_subject, 1).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42206);
}

#[tokio::test]
async fn hard_delete_unreferenced_version_still_works() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("ref-unref-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    // Soft-delete then hard-delete — no references, should succeed.
    common::api::delete_version(&client, &base, &subject, "1").await;
    let resp = common::api::hard_delete_version(&client, &base, &subject, 1).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn hard_delete_subject_with_referenced_version_returns_422() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let ref_subject = format!("ref-subjprot-{}", uuid::Uuid::new_v4());
    let dep_subject = format!("ref-subjprot-dep-{}", uuid::Uuid::new_v4());

    // Register base + dependent with reference.
    common::api::register_schema(&client, &base, &ref_subject, common::AVRO_SCHEMA_V1).await;
    let refs = serde_json::json!([
        {"name": "Base", "subject": ref_subject, "version": 1}
    ]);
    common::api::register_schema_with_refs(&client, &base, &dep_subject, common::AVRO_SCHEMA_V2, &refs).await;

    // Soft-delete the subject first.
    common::api::delete_subject(&client, &base, &ref_subject).await;

    // Hard-delete subject should be blocked because v1 is referenced.
    let resp = common::api::hard_delete_subject(&client, &base, &ref_subject).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42206);
}

#[tokio::test]
async fn hard_delete_dependent_then_referenced_succeeds() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let ref_subject = format!("ref-chain-{}", uuid::Uuid::new_v4());
    let dep_subject = format!("ref-chain-dep-{}", uuid::Uuid::new_v4());

    // Register base + dependent with reference.
    common::api::register_schema(&client, &base, &ref_subject, common::AVRO_SCHEMA_V1).await;
    let refs = serde_json::json!([
        {"name": "Base", "subject": ref_subject, "version": 1}
    ]);
    common::api::register_schema_with_refs(&client, &base, &dep_subject, common::AVRO_SCHEMA_V2, &refs).await;

    // Hard-delete the dependent first (soft-delete → hard-delete).
    common::api::delete_subject(&client, &base, &dep_subject).await;
    let resp = common::api::hard_delete_subject(&client, &base, &dep_subject).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Now the referenced version is no longer referenced — hard-delete should work.
    common::api::delete_version(&client, &base, &ref_subject, "1").await;
    let resp = common::api::hard_delete_version(&client, &base, &ref_subject, 1).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn soft_delete_referenced_version_still_works() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let ref_subject = format!("ref-soft-{}", uuid::Uuid::new_v4());
    let dep_subject = format!("ref-soft-dep-{}", uuid::Uuid::new_v4());

    // Register base + dependent with reference.
    common::api::register_schema(&client, &base, &ref_subject, common::AVRO_SCHEMA_V1).await;
    let refs = serde_json::json!([
        {"name": "Base", "subject": ref_subject, "version": 1}
    ]);
    common::api::register_schema_with_refs(&client, &base, &dep_subject, common::AVRO_SCHEMA_V2, &refs).await;

    // Soft-delete should work — only hard-delete is blocked.
    let resp = common::api::delete_version(&client, &base, &ref_subject, "1").await;
    assert_eq!(resp.status(), StatusCode::OK);
}
