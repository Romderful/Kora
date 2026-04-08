//! Integration tests for schema retrieval by subject and version
//! (GET /subjects/{subject}/versions/{version}).

mod common;

use reqwest::StatusCode;

#[tokio::test]
async fn get_specific_version() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("ver-{}", uuid::Uuid::new_v4());

    let id1 = common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V2).await;

    let resp = common::api::get_schema_by_version(&client, &base, &subject, "1").await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["subject"], subject);
    assert_eq!(body["id"], id1);
    assert_eq!(body["version"], 1);
    assert_eq!(body["schema"], common::AVRO_SCHEMA_V1);
    assert_eq!(body["schemaType"], "AVRO");
}

#[tokio::test]
async fn get_latest_version() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("latest-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    let id2 = common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V2).await;

    let resp = common::api::get_schema_by_version(&client, &base, &subject, "latest").await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["id"], id2);
    assert_eq!(body["version"], 2);
    assert_eq!(body["schema"], common::AVRO_SCHEMA_V2);
}

#[tokio::test]
async fn unknown_subject_returns_40401() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = common::api::get_schema_by_version(&client, &base, "nonexistent", "1").await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40401);
}

#[tokio::test]
async fn unknown_version_returns_40402() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("ver404-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = common::api::get_schema_by_version(&client, &base, &subject, "99").await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40402);
}

#[tokio::test]
async fn negative_version_returns_40402() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("neg-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = common::api::get_schema_by_version(&client, &base, &subject, "-1").await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40402);
}

#[tokio::test]
async fn zero_version_returns_40402() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("zero-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = common::api::get_schema_by_version(&client, &base, &subject, "0").await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40402);
}

#[tokio::test]
async fn all_schemas_soft_deleted_latest_returns_40402() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("softdel-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::delete_version(&client, &base, &subject, "1").await;

    let resp = common::api::get_schema_by_version(&client, &base, &subject, "latest").await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40402);
}
