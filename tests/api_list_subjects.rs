//! Integration tests for listing subjects (GET /subjects)
//! and listing versions (GET /subjects/{subject}/versions).

mod common;

use reqwest::StatusCode;

#[tokio::test]
async fn list_subjects_returns_registered_names() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let s1 = format!("list-a-{}", uuid::Uuid::new_v4());
    let s2 = format!("list-b-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &s1, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &s2, common::AVRO_SCHEMA_V1).await;

    let names = common::api::list_subjects(&client, &base, common::ACTIVE_ONLY).await;
    assert!(names.contains(&s1));
    assert!(names.contains(&s2));
}

#[tokio::test]
async fn list_versions_returns_sorted_versions() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("versions-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V2).await;

    let versions = common::api::list_versions(&client, &base, &subject, common::ACTIVE_ONLY).await;
    assert_eq!(versions, vec![1, 2]);
}

#[tokio::test]
async fn list_subjects_with_pagination() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    // limit=-1 (unlimited, default) returns 200.
    let resp = client
        .get(format!("{base}/subjects?limit=-1"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Large offset returns empty.
    let resp = client
        .get(format!("{base}/subjects?offset=999999"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let empty: Vec<String> = resp.json().await.unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn list_versions_with_pagination() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("pag-ver-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V2).await;
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V3).await;

    // First 2 versions.
    let resp = client
        .get(format!(
            "{base}/subjects/{subject}/versions?offset=0&limit=2"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let versions: Vec<i32> = resp.json().await.unwrap();
    assert_eq!(versions, vec![1, 2]);

    // Skip first, get rest.
    let resp = client
        .get(format!(
            "{base}/subjects/{subject}/versions?offset=1&limit=10"
        ))
        .send()
        .await
        .unwrap();
    let versions: Vec<i32> = resp.json().await.unwrap();
    assert_eq!(versions, vec![2, 3]);
}

#[tokio::test]
async fn list_subjects_with_subject_prefix_filters() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let prefix = format!("pfx-{}", uuid::Uuid::new_v4());
    let s1 = format!("{prefix}-alpha");
    let s2 = format!("{prefix}-beta");
    let other = format!("other-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &s1, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &s2, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &other, common::AVRO_SCHEMA_V1).await;

    let resp = client
        .get(format!("{base}/subjects?subjectPrefix={prefix}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let names: Vec<String> = resp.json().await.unwrap();

    assert!(names.contains(&s1));
    assert!(names.contains(&s2));
    assert!(!names.contains(&other));
}

#[tokio::test]
async fn list_subjects_deleted_only_returns_only_soft_deleted() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let active = format!("active-{}", uuid::Uuid::new_v4());
    let deleted = format!("deleted-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &active, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &deleted, common::AVRO_SCHEMA_V1).await;
    common::api::delete_subject(&client, &base, &deleted).await;

    let resp = client
        .get(format!("{base}/subjects?deletedOnly=true"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let names: Vec<String> = resp.json().await.unwrap();

    assert!(
        names.contains(&deleted),
        "soft-deleted subject should appear"
    );
    assert!(
        !names.contains(&active),
        "active subject should NOT appear with deletedOnly"
    );
}

#[tokio::test]
async fn list_subjects_deleted_only_takes_precedence_over_deleted() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let active = format!("active-prec-{}", uuid::Uuid::new_v4());
    let deleted = format!("deleted-prec-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &active, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &deleted, common::AVRO_SCHEMA_V1).await;
    common::api::delete_subject(&client, &base, &deleted).await;

    let resp = client
        .get(format!("{base}/subjects?deleted=true&deletedOnly=true"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let names: Vec<String> = resp.json().await.unwrap();

    assert!(
        names.contains(&deleted),
        "soft-deleted subject should appear"
    );
    assert!(
        !names.contains(&active),
        "active subject should NOT appear — deletedOnly takes precedence"
    );
}

#[tokio::test]
async fn list_subjects_subject_prefix_empty_string_returns_all() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let s1 = format!("empty-pfx-{}", uuid::Uuid::new_v4());
    common::api::register_schema(&client, &base, &s1, common::AVRO_SCHEMA_V1).await;

    let resp = client
        .get(format!("{base}/subjects?subjectPrefix="))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let names: Vec<String> = resp.json().await.unwrap();
    assert!(names.contains(&s1));
}

#[tokio::test]
async fn list_subjects_lookup_deleted_subject_is_accepted() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{base}/subjects?lookupDeletedSubject=true"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn list_versions_deleted_only_returns_only_soft_deleted() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("ver-do-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V2).await;
    common::api::delete_version(&client, &base, &subject, "1").await;

    let resp = client
        .get(format!(
            "{base}/subjects/{subject}/versions?deletedOnly=true"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let versions: Vec<i32> = resp.json().await.unwrap();

    assert_eq!(
        versions,
        vec![1],
        "only soft-deleted version 1 should appear"
    );
}

#[tokio::test]
async fn list_versions_deleted_as_negative() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("ver-dan-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V2).await;
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V3).await;
    common::api::delete_version(&client, &base, &subject, "2").await;

    let resp = client
        .get(format!(
            "{base}/subjects/{subject}/versions?deleted=true&deletedAsNegative=true"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let versions: Vec<i32> = resp.json().await.unwrap();

    assert_eq!(versions, vec![1, -2, 3]);
}

#[tokio::test]
async fn list_versions_deleted_only_with_deleted_as_negative() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("ver-do-dan-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V2).await;
    // Soft-delete version 1.
    common::api::delete_version(&client, &base, &subject, "1").await;

    // deletedOnly=true + deletedAsNegative=true → deleted versions as negative.
    let resp = client
        .get(format!(
            "{base}/subjects/{subject}/versions?deletedOnly=true&deletedAsNegative=true"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let versions: Vec<i32> = resp.json().await.unwrap();

    assert_eq!(
        versions,
        vec![-1],
        "deletedOnly + deletedAsNegative should return negative versions"
    );
}

#[tokio::test]
async fn list_versions_unknown_subject_returns_40401() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    // Must be inline — helper unwraps json and can't return error responses.
    let resp = client
        .get(format!("{base}/subjects/nonexistent/versions"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40401);
}

#[tokio::test]
async fn list_versions_soft_deleted_subject_with_deleted_returns_versions() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("sdel-ver-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V2).await;
    common::api::delete_subject(&client, &base, &subject).await;

    // Without deleted → 40401 (soft-deleted subject).
    let resp = client
        .get(format!("{base}/subjects/{subject}/versions"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // With deleted=true → returns soft-deleted versions.
    let resp = client
        .get(format!("{base}/subjects/{subject}/versions?deleted=true"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let versions: Vec<i32> = resp.json().await.unwrap();
    assert_eq!(versions, vec![1, 2]);
}

// -- Python-style booleans (case-insensitive query params) --

#[tokio::test]
async fn list_subjects_accepts_python_style_deleted_true() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("py-del-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::delete_subject(&client, &base, &subject).await;

    // Python's str(True) → "True"
    let resp = client
        .get(format!("{base}/subjects?deleted=True"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let names: Vec<String> = resp.json().await.unwrap();
    assert!(names.contains(&subject));
}

#[tokio::test]
async fn list_subjects_accepts_uppercase_deleted_true() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{base}/subjects?deleted=TRUE"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn list_versions_accepts_python_style_booleans() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("py-ver-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V2).await;
    common::api::delete_version(&client, &base, &subject, "1").await;

    let resp = client
        .get(format!(
            "{base}/subjects/{subject}/versions?deleted=True&deletedAsNegative=True"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let versions: Vec<i32> = resp.json().await.unwrap();
    assert_eq!(versions, vec![-1, 2]);
}
