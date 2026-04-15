//! Integration tests for schema retrieval by subject and version
//! (GET /subjects/{subject}/versions/{version}).

mod common;

use reqwest::StatusCode;

#[tokio::test]
async fn get_schema_by_version_specific_succeeds() {
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
async fn get_schema_by_version_latest_succeeds() {
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
async fn get_schema_by_version_unknown_subject_returns_40401() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = common::api::get_schema_by_version(&client, &base, "nonexistent", "1").await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40401);
}

#[tokio::test]
async fn get_schema_by_version_unknown_returns_40402() {
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
async fn get_schema_by_version_negative_returns_42202() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("neg-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = common::api::get_schema_by_version(&client, &base, &subject, "-1").await;

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42202);
}

#[tokio::test]
async fn get_schema_by_version_zero_returns_42202() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("zero-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = common::api::get_schema_by_version(&client, &base, &subject, "0").await;

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42202);
}

#[tokio::test]
async fn get_schema_by_version_non_numeric_returns_42202() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("abc-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = common::api::get_schema_by_version(&client, &base, &subject, "abc").await;

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42202);
}

#[tokio::test]
async fn get_schema_by_version_accepts_format_params() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("fmt-ver-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = client
        .get(format!(
            "{base}/subjects/{subject}/versions/1?format=serialized&referenceFormat=DEFAULT"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn get_schema_by_version_all_deleted_latest_returns_40402() {
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

#[tokio::test]
async fn get_schema_by_version_deleted_returns_soft_deleted() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("ver-del-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V2).await;
    common::api::delete_version(&client, &base, &subject, "2").await;

    // Without deleted → 404.
    let resp = common::api::get_schema_by_version(&client, &base, &subject, "2").await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // With deleted=true → returns the soft-deleted version.
    let resp = client
        .get(format!("{base}/subjects/{subject}/versions/2?deleted=true"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "deleted=true should return soft-deleted version"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["version"], 2);
}

#[tokio::test]
async fn get_schema_by_version_latest_with_deleted_returns_soft_deleted() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("latest-del-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V2).await;
    // Soft-delete version 2 (the latest).
    common::api::delete_version(&client, &base, &subject, "2").await;

    // latest without deleted → returns version 1 (the latest active).
    let resp = common::api::get_schema_by_version(&client, &base, &subject, "latest").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["version"], 1);

    // latest with deleted=true → returns version 2 (soft-deleted but highest).
    let resp = client
        .get(format!(
            "{base}/subjects/{subject}/versions/latest?deleted=true"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "latest?deleted=true should consider soft-deleted versions"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["version"], 2);
}

// -- Raw schema text by version (GET /subjects/{subject}/versions/{version}/schema) --

#[tokio::test]
async fn raw_schema_by_version_returns_text() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("raw-ver-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = client
        .get(format!("{base}/subjects/{subject}/versions/1/schema"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let text: String = resp.json().await.unwrap();
    assert_eq!(text, common::AVRO_SCHEMA_V1);
}

#[tokio::test]
async fn raw_schema_by_version_latest() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("raw-latest-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V2).await;

    let resp = client
        .get(format!("{base}/subjects/{subject}/versions/latest/schema"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let text: String = resp.json().await.unwrap();
    assert_eq!(text, common::AVRO_SCHEMA_V2);
}

#[tokio::test]
async fn raw_schema_by_version_soft_deleted_without_param_returns_40402() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("raw-softdel-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::delete_version(&client, &base, &subject, "1").await;

    let resp = client
        .get(format!("{base}/subjects/{subject}/versions/1/schema"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40402);
}

#[tokio::test]
async fn raw_schema_by_version_soft_deleted_with_param_returns_text() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("raw-del-ok-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::delete_version(&client, &base, &subject, "1").await;

    let resp = client
        .get(format!(
            "{base}/subjects/{subject}/versions/1/schema?deleted=true"
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let text: String = resp.json().await.unwrap();
    assert_eq!(text, common::AVRO_SCHEMA_V1);
}

#[tokio::test]
async fn raw_schema_by_version_nonexistent_subject_returns_40401() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{base}/subjects/nonexistent/versions/1/schema"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40401);
}

#[tokio::test]
async fn raw_schema_by_version_nonexistent_version_returns_40402() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("raw-noexist-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = client
        .get(format!("{base}/subjects/{subject}/versions/99/schema"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40402);
}

#[tokio::test]
async fn raw_schema_by_version_invalid_version_returns_42202() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("raw-invalid-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = client
        .get(format!("{base}/subjects/{subject}/versions/0/schema"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 42202);
}
