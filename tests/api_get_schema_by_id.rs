//! Integration tests for schema retrieval by global ID (GET /schemas/ids/{id}).

mod common;

use reqwest::StatusCode;

#[tokio::test]
async fn get_schema_by_id_succeeds() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("get-by-id-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = common::api::get_schema_by_id(&client, &base, id).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["schema"].as_str().unwrap(), common::AVRO_SCHEMA_V1);
    assert_eq!(body["schemaType"], "AVRO");
}

#[tokio::test]
async fn get_schema_by_id_nonexistent_returns_404() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = common::api::get_schema_by_id(&client, &base, i64::MAX).await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40403);
}

#[tokio::test]
async fn get_schema_by_id_soft_deleted_still_returns_200() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("soft-del-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;
    common::api::delete_version(&client, &base, &subject, "1").await;

    let resp = common::api::get_schema_by_id(&client, &base, id).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["schema"].as_str().unwrap(), common::AVRO_SCHEMA_V1);
}

#[tokio::test]
async fn get_schema_by_id_includes_schema_type_for_json() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("json-type-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema_with_type(
        &client,
        &base,
        &subject,
        common::JSON_SCHEMA_V1,
        "JSON",
    )
    .await;

    let resp = common::api::get_schema_by_id(&client, &base, id).await;
    let body: serde_json::Value = resp.json().await.unwrap();

    assert_eq!(body["schemaType"], "JSON");
}

#[tokio::test]
async fn get_schema_by_id_with_fetch_max_id() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("maxid-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = client
        .get(format!("{base}/schemas/ids/{id}?fetchMaxId=true"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();

    assert!(
        body["maxId"].is_number(),
        "maxId should be present when fetchMaxId=true"
    );
    assert!(body["maxId"].as_i64().unwrap() >= id);
}

#[tokio::test]
async fn get_schema_by_id_accepts_format_and_subject_params() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("fmt-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = client
        .get(format!(
            "{base}/schemas/ids/{id}?format=serialized&subject={subject}&referenceFormat=DEFAULT"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn get_schema_by_id_invalid_returns_400() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    // Invalid ID (non-numeric) — axum rejects with 400.
    let resp = client
        .get(format!("{base}/schemas/ids/abc"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// -- Raw schema text by ID (GET /schemas/ids/{id}/schema) --

#[tokio::test]
async fn raw_schema_by_id_returns_schema_text() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("raw-id-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = client
        .get(format!("{base}/schemas/ids/{id}/schema"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let text: String = resp.json().await.unwrap();
    assert_eq!(text, common::AVRO_SCHEMA_V1);
}

#[tokio::test]
async fn raw_schema_by_id_json_type() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("raw-json-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema_with_type(
        &client,
        &base,
        &subject,
        common::JSON_SCHEMA_V1,
        "JSON",
    )
    .await;

    let resp = client
        .get(format!("{base}/schemas/ids/{id}/schema"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let text: String = resp.json().await.unwrap();
    assert_eq!(text, common::JSON_SCHEMA_V1);
}

#[tokio::test]
async fn raw_schema_by_id_protobuf_type() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("raw-proto-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema_with_type(
        &client,
        &base,
        &subject,
        common::PROTO_SCHEMA_V1,
        "PROTOBUF",
    )
    .await;

    let resp = client
        .get(format!("{base}/schemas/ids/{id}/schema"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let text: String = resp.json().await.unwrap();
    assert_eq!(text, common::PROTO_SCHEMA_V1);
}

#[tokio::test]
async fn raw_schema_by_id_nonexistent_returns_40403() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{base}/schemas/ids/{}/schema", i64::MAX))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40403);
}

#[tokio::test]
async fn raw_schema_by_id_accepts_subject_param() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("raw-subj-{}", uuid::Uuid::new_v4());

    let id = common::api::register_schema(&client, &base, &subject, common::AVRO_SCHEMA_V1).await;

    let resp = client
        .get(format!("{base}/schemas/ids/{id}/schema?subject={subject}"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}
