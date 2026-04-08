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
