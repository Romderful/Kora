//! Integration tests for schema references and dependency protection (Story 2.4).
//!
//! Tests reference validation on registration and deletion protection.

mod common;

use reqwest::StatusCode;

// -- Registration with valid references (AC #1) --

#[tokio::test]
async fn register_schema_with_valid_ref_succeeds() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let ref_subject = format!("ref-base-{}", uuid::Uuid::new_v4());
    let dep_subject = format!("ref-dep-{}", uuid::Uuid::new_v4());

    // Register the referenced schema first.
    common::api::register_schema(&client, &base, &ref_subject, &common::unique_avro_schema()).await;

    // Register a schema that references it.
    let refs = serde_json::json!([
        {"name": "Base", "subject": ref_subject, "version": 1}
    ]);
    let resp = common::api::register_schema_with_refs(
        &client,
        &base,
        &dep_subject,
        &common::unique_avro_schema(),
        &refs,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["id"].as_i64().is_some());
}

#[tokio::test]
async fn register_schema_without_refs_succeeds() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("ref-norefs-{}", uuid::Uuid::new_v4());

    let id =
        common::api::register_schema(&client, &base, &subject, &common::unique_avro_schema()).await;
    assert!(id > 0);
}

// -- Registration with invalid references (AC #2) --

#[tokio::test]
async fn register_schema_ref_nonexistent_subject_returns_422() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("ref-badsubj-{}", uuid::Uuid::new_v4());

    let refs = serde_json::json!([
        {"name": "Missing", "subject": "nonexistent-subject", "version": 1}
    ]);
    let resp = common::api::register_schema_with_refs(
        &client,
        &base,
        &subject,
        &common::unique_avro_schema(),
        &refs,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42201);
}

#[tokio::test]
async fn register_schema_ref_nonexistent_version_returns_422() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let ref_subject = format!("ref-badver-{}", uuid::Uuid::new_v4());
    let dep_subject = format!("ref-badver-dep-{}", uuid::Uuid::new_v4());

    // Register v1 only.
    common::api::register_schema(&client, &base, &ref_subject, &common::unique_avro_schema()).await;

    // Reference v99 which doesn't exist.
    let refs = serde_json::json!([
        {"name": "Base", "subject": ref_subject, "version": 99}
    ]);
    let resp = common::api::register_schema_with_refs(
        &client,
        &base,
        &dep_subject,
        &common::unique_avro_schema(),
        &refs,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42201);
}

// -- Deletion protection (AC #3) --

#[tokio::test]
async fn hard_delete_version_referenced_returns_422() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let ref_subject = format!("ref-prot-{}", uuid::Uuid::new_v4());
    let dep_subject = format!("ref-prot-dep-{}", uuid::Uuid::new_v4());

    // Register base schema + dependent schema with reference.
    common::api::register_schema(&client, &base, &ref_subject, &common::unique_avro_schema()).await;
    let refs = serde_json::json!([
        {"name": "Base", "subject": ref_subject, "version": 1}
    ]);
    common::api::register_schema_with_refs(
        &client,
        &base,
        &dep_subject,
        &common::unique_avro_schema(),
        &refs,
    )
    .await;

    // Soft-delete the referenced version first (required before hard-delete).
    common::api::delete_version(&client, &base, &ref_subject, "1").await;

    // Hard-delete should be blocked.
    let resp = common::api::hard_delete_version(&client, &base, &ref_subject, 1).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42206);
}

#[tokio::test]
async fn hard_delete_version_unreferenced_succeeds() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("ref-unref-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, &common::unique_avro_schema()).await;

    // Soft-delete then hard-delete — no references, should succeed.
    common::api::delete_version(&client, &base, &subject, "1").await;
    let resp = common::api::hard_delete_version(&client, &base, &subject, 1).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn hard_delete_subject_with_ref_version_returns_422() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let ref_subject = format!("ref-subjprot-{}", uuid::Uuid::new_v4());
    let dep_subject = format!("ref-subjprot-dep-{}", uuid::Uuid::new_v4());

    // Register base + dependent with reference.
    common::api::register_schema(&client, &base, &ref_subject, &common::unique_avro_schema()).await;
    let refs = serde_json::json!([
        {"name": "Base", "subject": ref_subject, "version": 1}
    ]);
    common::api::register_schema_with_refs(
        &client,
        &base,
        &dep_subject,
        &common::unique_avro_schema(),
        &refs,
    )
    .await;

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
    common::api::register_schema(&client, &base, &ref_subject, &common::unique_avro_schema()).await;
    let refs = serde_json::json!([
        {"name": "Base", "subject": ref_subject, "version": 1}
    ]);
    common::api::register_schema_with_refs(
        &client,
        &base,
        &dep_subject,
        &common::unique_avro_schema(),
        &refs,
    )
    .await;

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
async fn soft_delete_version_referenced_succeeds() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let ref_subject = format!("ref-soft-{}", uuid::Uuid::new_v4());
    let dep_subject = format!("ref-soft-dep-{}", uuid::Uuid::new_v4());

    // Register base + dependent with reference.
    common::api::register_schema(&client, &base, &ref_subject, &common::unique_avro_schema()).await;
    let refs = serde_json::json!([
        {"name": "Base", "subject": ref_subject, "version": 1}
    ]);
    common::api::register_schema_with_refs(
        &client,
        &base,
        &dep_subject,
        &common::unique_avro_schema(),
        &refs,
    )
    .await;

    // Soft-delete should work — only hard-delete is blocked.
    let resp = common::api::delete_version(&client, &base, &ref_subject, "1").await;
    assert_eq!(resp.status(), StatusCode::OK);
}

// -- Referenced-by lookup (GET /subjects/{subject}/versions/{version}/referencedby) --

#[tokio::test]
async fn referencedby_returns_referencing_schema_ids() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let ref_subject = format!("refby-base-{}", uuid::Uuid::new_v4());
    let dep_subject = format!("refby-dep-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &ref_subject, &common::unique_avro_schema()).await;
    let refs = serde_json::json!([{"name": "Base", "subject": ref_subject, "version": 1}]);
    let resp = common::api::register_schema_with_refs(
        &client,
        &base,
        &dep_subject,
        &common::unique_avro_schema(),
        &refs,
    )
    .await;
    let dep_id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();

    let resp = client
        .get(format!(
            "{base}/subjects/{ref_subject}/versions/1/referencedby"
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let ids: Vec<i64> = resp.json().await.unwrap();
    assert!(ids.contains(&dep_id));
}

#[tokio::test]
async fn referencedby_no_dependents_returns_empty() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("refby-empty-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, &common::unique_avro_schema()).await;

    let resp = client
        .get(format!("{base}/subjects/{subject}/versions/1/referencedby"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let ids: Vec<i64> = resp.json().await.unwrap();
    assert!(ids.is_empty());
}

#[tokio::test]
async fn referencedby_multiple_dependents() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let ref_subject = format!("refby-multi-{}", uuid::Uuid::new_v4());
    let dep1 = format!("refby-multi-dep1-{}", uuid::Uuid::new_v4());
    let dep2 = format!("refby-multi-dep2-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &ref_subject, &common::unique_avro_schema()).await;
    let refs = serde_json::json!([{"name": "Base", "subject": ref_subject, "version": 1}]);

    let resp = common::api::register_schema_with_refs(
        &client,
        &base,
        &dep1,
        &common::unique_avro_schema(),
        &refs,
    )
    .await;
    let dep1_id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();

    let resp = common::api::register_schema_with_refs(
        &client,
        &base,
        &dep2,
        &common::unique_avro_schema(),
        &refs,
    )
    .await;
    let dep2_id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();

    let resp = client
        .get(format!(
            "{base}/subjects/{ref_subject}/versions/1/referencedby"
        ))
        .send()
        .await
        .unwrap();

    let ids: Vec<i64> = resp.json().await.unwrap();
    assert!(ids.contains(&dep1_id));
    assert!(ids.contains(&dep2_id));
    assert_eq!(ids.len(), 2);
}

#[tokio::test]
async fn referencedby_excludes_soft_deleted_by_default() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let ref_subject = format!("refby-softdel-{}", uuid::Uuid::new_v4());
    let dep_subject = format!("refby-softdel-dep-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &ref_subject, &common::unique_avro_schema()).await;
    let refs = serde_json::json!([{"name": "Base", "subject": ref_subject, "version": 1}]);
    common::api::register_schema_with_refs(
        &client,
        &base,
        &dep_subject,
        &common::unique_avro_schema(),
        &refs,
    )
    .await;
    common::api::delete_version(&client, &base, &dep_subject, "1").await;

    let resp = client
        .get(format!(
            "{base}/subjects/{ref_subject}/versions/1/referencedby"
        ))
        .send()
        .await
        .unwrap();

    let ids: Vec<i64> = resp.json().await.unwrap();
    assert!(
        ids.is_empty(),
        "soft-deleted referencing schema should be excluded by default"
    );
}

#[tokio::test]
async fn referencedby_includes_soft_deleted_with_param() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let ref_subject = format!("refby-del-inc-{}", uuid::Uuid::new_v4());
    let dep_subject = format!("refby-del-inc-dep-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &ref_subject, &common::unique_avro_schema()).await;
    let refs = serde_json::json!([{"name": "Base", "subject": ref_subject, "version": 1}]);
    let resp = common::api::register_schema_with_refs(
        &client,
        &base,
        &dep_subject,
        &common::unique_avro_schema(),
        &refs,
    )
    .await;
    let dep_id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();
    common::api::delete_version(&client, &base, &dep_subject, "1").await;

    let resp = client
        .get(format!(
            "{base}/subjects/{ref_subject}/versions/1/referencedby?deleted=true"
        ))
        .send()
        .await
        .unwrap();

    let ids: Vec<i64> = resp.json().await.unwrap();
    assert!(
        ids.contains(&dep_id),
        "soft-deleted should be included with deleted=true"
    );
}

#[tokio::test]
async fn referencedby_pagination() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let ref_subject = format!("refby-page-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &ref_subject, &common::unique_avro_schema()).await;
    let refs = serde_json::json!([{"name": "Base", "subject": ref_subject, "version": 1}]);

    for i in 0..3 {
        let dep = format!("refby-page-dep{i}-{}", uuid::Uuid::new_v4());
        common::api::register_schema_with_refs(
            &client,
            &base,
            &dep,
            &common::unique_avro_schema(),
            &refs,
        )
        .await;
    }

    let resp = client
        .get(format!(
            "{base}/subjects/{ref_subject}/versions/1/referencedby?offset=1&limit=1"
        ))
        .send()
        .await
        .unwrap();

    let ids: Vec<i64> = resp.json().await.unwrap();
    assert_eq!(ids.len(), 1);
}

#[tokio::test]
async fn referencedby_nonexistent_subject_returns_40401() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!(
            "{base}/subjects/nonexistent/versions/1/referencedby"
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40401);
}

#[tokio::test]
async fn referencedby_nonexistent_version_returns_40402() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("refby-nover-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, &common::unique_avro_schema()).await;

    let resp = client
        .get(format!(
            "{base}/subjects/{subject}/versions/99/referencedby"
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40402);
}

#[tokio::test]
async fn referencedby_invalid_version_returns_42202() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("refby-badver-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, &common::unique_avro_schema()).await;

    let resp = client
        .get(format!("{base}/subjects/{subject}/versions/0/referencedby"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42202);
}
