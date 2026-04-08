//! Integration tests for soft-deleting subjects and versions
//! (DELETE /subjects/{subject}, DELETE /subjects/{subject}/versions/{version}).

mod common;

use reqwest::StatusCode;

#[tokio::test]
async fn delete_subject_returns_versions() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("del-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V2).await;
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V3).await;

    let resp = common::api::delete_subject(&client, &base, &subject).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Vec<i32> = resp.json().await.unwrap();
    assert_eq!(body, vec![1, 2, 3]);
}

#[tokio::test]
async fn deleted_subject_excluded_from_list() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("del-list-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::delete_subject(&client, &base, &subject).await;

    let names = common::api::list_subjects(&client, &base, common::ACTIVE_ONLY).await;
    assert!(!names.contains(&subject));
}

#[tokio::test]
async fn deleted_true_includes_all_subjects() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let active = format!("del-active-{}", uuid::Uuid::new_v4());
    let deleted = format!("del-gone-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &active, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &deleted, common::AVRO_SCHEMA_V1).await;
    common::api::delete_subject(&client, &base, &deleted).await;

    let names = common::api::list_subjects(&client, &base, common::INCLUDE_DELETED).await;
    assert!(names.contains(&active), "active subject missing from ?deleted=true");
    assert!(names.contains(&deleted), "deleted subject missing from ?deleted=true");

    let names = common::api::list_subjects(&client, &base, common::ACTIVE_ONLY).await;
    assert!(names.contains(&active), "active subject missing from default list");
    assert!(!names.contains(&deleted), "deleted subject should NOT appear in default list");
}

#[tokio::test]
async fn delete_single_version() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("del-ver-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V2).await;
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V3).await;

    let resp = common::api::delete_version(&client, &base, &subject, "2").await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body: i32 = resp.json().await.unwrap();
    assert_eq!(body, 2);

    let versions = common::api::list_versions(&client, &base, &subject, common::ACTIVE_ONLY).await;
    assert_eq!(versions, vec![1, 3]);
}

#[tokio::test]
async fn deleted_versions_listed_with_deleted_flag() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("del-ver-flag-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V2).await;
    common::api::delete_version(&client, &base, &subject, "2").await;

    let versions = common::api::list_versions(&client, &base, &subject, common::ACTIVE_ONLY).await;
    assert_eq!(versions, vec![1]);

    let versions = common::api::list_versions(&client, &base, &subject, common::INCLUDE_DELETED).await;
    assert_eq!(versions, vec![1, 2]);
}

#[tokio::test]
async fn delete_already_deleted_subject_returns_40401() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("del-twice-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = common::api::delete_subject(&client, &base, &subject).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = common::api::delete_subject(&client, &base, &subject).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40401);
}

#[tokio::test]
async fn delete_nonexistent_subject_returns_40401() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = common::api::delete_subject(&client, &base, "nonexistent").await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40401);
}

#[tokio::test]
async fn delete_nonexistent_version_returns_40402() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("del-ver404-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = common::api::delete_version(&client, &base, &subject, "99").await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40402);
}

#[tokio::test]
async fn delete_latest_version() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("del-latest-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V2).await;

    let resp = common::api::delete_version(&client, &base, &subject, "latest").await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body: i32 = resp.json().await.unwrap();
    assert_eq!(body, 2);

    let versions = common::api::list_versions(&client, &base, &subject, common::ACTIVE_ONLY).await;
    assert_eq!(versions, vec![1]);
}
