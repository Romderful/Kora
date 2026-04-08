//! Integration tests for soft-deleting subjects and versions
//! (DELETE /subjects/{subject}, DELETE /subjects/{subject}/versions/{version}).

mod common;

use reqwest::StatusCode;

const VALID_AVRO: &str = r#"{"type":"record","name":"Test","fields":[{"name":"id","type":"int"}]}"#;
const AVRO_V2: &str =
    r#"{"type":"record","name":"Test","fields":[{"name":"id","type":"int"},{"name":"name","type":"string"}]}"#;
const AVRO_V3: &str =
    r#"{"type":"record","name":"Test","fields":[{"name":"id","type":"int"},{"name":"name","type":"string"},{"name":"active","type":"boolean"}]}"#;

async fn register(client: &reqwest::Client, base: &str, subject: &str, schema: &str) {
    client
        .post(format!("{base}/subjects/{subject}/versions"))
        .json(&serde_json::json!({"schema": schema}))
        .send()
        .await
        .unwrap();
}

#[tokio::test]
async fn delete_subject_returns_versions() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("del-{}", uuid::Uuid::new_v4());

    register(&client, &base, &subject, VALID_AVRO).await;
    register(&client, &base, &subject, AVRO_V2).await;
    register(&client, &base, &subject, AVRO_V3).await;

    let resp = client
        .delete(format!("{base}/subjects/{subject}"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body: Vec<i32> = resp.json().await.unwrap();
    assert_eq!(body, vec![1, 2, 3]);
}

#[tokio::test]
async fn deleted_subject_excluded_from_list() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("del-list-{}", uuid::Uuid::new_v4());

    register(&client, &base, &subject, VALID_AVRO).await;

    client
        .delete(format!("{base}/subjects/{subject}"))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!("{base}/subjects"))
        .send()
        .await
        .unwrap();
    let names: Vec<String> = resp.json().await.unwrap();
    assert!(!names.contains(&subject));
}

#[tokio::test]
async fn deleted_true_includes_all_subjects() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let active = format!("del-active-{}", uuid::Uuid::new_v4());
    let deleted = format!("del-gone-{}", uuid::Uuid::new_v4());

    register(&client, &base, &active, VALID_AVRO).await;
    register(&client, &base, &deleted, VALID_AVRO).await;

    client
        .delete(format!("{base}/subjects/{deleted}"))
        .send()
        .await
        .unwrap();

    // ?deleted=true includes ALL subjects (active + deleted).
    let resp = client
        .get(format!("{base}/subjects?deleted=true"))
        .send()
        .await
        .unwrap();
    let names: Vec<String> = resp.json().await.unwrap();
    assert!(names.contains(&active), "active subject missing from ?deleted=true");
    assert!(names.contains(&deleted), "deleted subject missing from ?deleted=true");

    // Default (no flag) returns ONLY active subjects.
    let resp = client
        .get(format!("{base}/subjects"))
        .send()
        .await
        .unwrap();
    let names: Vec<String> = resp.json().await.unwrap();
    assert!(names.contains(&active), "active subject missing from default list");
    assert!(!names.contains(&deleted), "deleted subject should NOT appear in default list");
}

#[tokio::test]
async fn delete_single_version() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("del-ver-{}", uuid::Uuid::new_v4());

    register(&client, &base, &subject, VALID_AVRO).await;
    register(&client, &base, &subject, AVRO_V2).await;
    register(&client, &base, &subject, AVRO_V3).await;

    let resp = client
        .delete(format!("{base}/subjects/{subject}/versions/2"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: i32 = resp.json().await.unwrap();
    assert_eq!(body, 2);

    // Remaining versions should be [1, 3].
    let resp = client
        .get(format!("{base}/subjects/{subject}/versions"))
        .send()
        .await
        .unwrap();
    let versions: Vec<i32> = resp.json().await.unwrap();
    assert_eq!(versions, vec![1, 3]);
}

#[tokio::test]
async fn deleted_versions_listed_with_deleted_flag() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("del-ver-flag-{}", uuid::Uuid::new_v4());

    register(&client, &base, &subject, VALID_AVRO).await;
    register(&client, &base, &subject, AVRO_V2).await;

    // Delete version 2.
    client
        .delete(format!("{base}/subjects/{subject}/versions/2"))
        .send()
        .await
        .unwrap();

    // Default: only active versions.
    let resp = client
        .get(format!("{base}/subjects/{subject}/versions"))
        .send()
        .await
        .unwrap();
    let versions: Vec<i32> = resp.json().await.unwrap();
    assert_eq!(versions, vec![1]);

    // ?deleted=true: all versions (active + deleted).
    let resp = client
        .get(format!("{base}/subjects/{subject}/versions?deleted=true"))
        .send()
        .await
        .unwrap();
    let versions: Vec<i32> = resp.json().await.unwrap();
    assert_eq!(versions, vec![1, 2]);
}

#[tokio::test]
async fn delete_already_deleted_subject_returns_40401() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("del-twice-{}", uuid::Uuid::new_v4());

    register(&client, &base, &subject, VALID_AVRO).await;

    // First delete succeeds.
    let resp = client
        .delete(format!("{base}/subjects/{subject}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Second delete should return 40401 — subject is already soft-deleted.
    let resp = client
        .delete(format!("{base}/subjects/{subject}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40401);
}

#[tokio::test]
async fn delete_nonexistent_subject_returns_40401() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .delete(format!("{base}/subjects/nonexistent"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40401);
}

#[tokio::test]
async fn delete_nonexistent_version_returns_40402() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("del-ver404-{}", uuid::Uuid::new_v4());

    register(&client, &base, &subject, VALID_AVRO).await;

    let resp = client
        .delete(format!("{base}/subjects/{subject}/versions/99"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40402);
}

#[tokio::test]
async fn delete_latest_version() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("del-latest-{}", uuid::Uuid::new_v4());

    register(&client, &base, &subject, VALID_AVRO).await;
    register(&client, &base, &subject, AVRO_V2).await;

    let resp = client
        .delete(format!("{base}/subjects/{subject}/versions/latest"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: i32 = resp.json().await.unwrap();
    assert_eq!(body, 2);

    // Remaining versions should be [1].
    let resp = client
        .get(format!("{base}/subjects/{subject}/versions"))
        .send()
        .await
        .unwrap();
    let versions: Vec<i32> = resp.json().await.unwrap();
    assert_eq!(versions, vec![1]);
}
