//! Integration tests for checking schema registration
//! (POST /subjects/{subject}).

mod common;

use reqwest::StatusCode;

#[tokio::test]
async fn check_registered_schema_returns_200() {
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
async fn check_unregistered_schema_returns_40403() {
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
async fn check_on_nonexistent_subject_returns_40401() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = common::api::check_schema(&client, &base, "nonexistent", common::AVRO_SCHEMA_V1).await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40401);
}
