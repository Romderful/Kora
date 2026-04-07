//! Integration tests for listing subjects (GET /subjects)
//! and listing versions (GET /subjects/{subject}/versions).

mod common;

use reqwest::StatusCode;

const VALID_AVRO: &str = r#"{"type":"record","name":"Test","fields":[{"name":"id","type":"int"}]}"#;
const AVRO_V2: &str =
    r#"{"type":"record","name":"Test","fields":[{"name":"id","type":"int"},{"name":"name","type":"string"}]}"#;

async fn register(client: &reqwest::Client, base: &str, subject: &str, schema: &str) {
    client
        .post(format!("{base}/subjects/{subject}/versions"))
        .json(&serde_json::json!({"schema": schema}))
        .send()
        .await
        .unwrap();
}

#[tokio::test]
async fn list_subjects_returns_registered_names() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let s1 = format!("list-a-{}", uuid::Uuid::new_v4());
    let s2 = format!("list-b-{}", uuid::Uuid::new_v4());

    register(&client, &base, &s1, VALID_AVRO).await;
    register(&client, &base, &s2, VALID_AVRO).await;

    let resp = client
        .get(format!("{base}/subjects"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body: Vec<String> = resp.json().await.unwrap();
    assert!(body.contains(&s1));
    assert!(body.contains(&s2));
}

#[tokio::test]
async fn list_subjects_empty_registry() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{base}/subjects"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_array().unwrap().is_empty() || body.is_array());
}

#[tokio::test]
async fn list_versions_returns_sorted_versions() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("versions-{}", uuid::Uuid::new_v4());

    register(&client, &base, &subject, VALID_AVRO).await;
    register(&client, &base, &subject, AVRO_V2).await;

    let resp = client
        .get(format!("{base}/subjects/{subject}/versions"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body: Vec<i32> = resp.json().await.unwrap();
    assert_eq!(body, vec![1, 2]);
}

#[tokio::test]
async fn list_versions_unknown_subject_returns_40401() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{base}/subjects/nonexistent/versions"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40401);
}
