//! Integration tests for listing all schemas (GET /schemas).

mod common;

use reqwest::StatusCode;

#[tokio::test]
async fn list_schemas_returns_all_versions() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let s1 = format!("list-all-a-{}", uuid::Uuid::new_v4());
    let s2 = format!("list-all-b-{}", uuid::Uuid::new_v4());

    common::api::register_schema(&client, &base, &s1, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &s1, common::AVRO_SCHEMA_V2).await;
    common::api::register_schema(&client, &base, &s2, common::AVRO_SCHEMA_V1).await;

    let resp = client.get(format!("{base}/schemas")).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Vec<serde_json::Value> = resp.json().await.unwrap();
    let our: Vec<_> = body
        .iter()
        .filter(|v| {
            let subj = v["subject"].as_str().unwrap_or("");
            subj == s1 || subj == s2
        })
        .collect();

    // s1 has 2 versions, s2 has 1 = 3 total.
    assert_eq!(our.len(), 3);

    // Each entry has subject, id, version, schema.
    for entry in &our {
        assert!(entry["subject"].is_string());
        assert!(entry["id"].is_number());
        assert!(entry["version"].is_number());
        assert!(entry["schema"].is_string());
    }
}

#[tokio::test]
async fn list_schemas_empty_returns_empty_array() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    // Use a prefix that won't match any schema.
    let resp = client
        .get(format!(
            "{base}/schemas?subjectPrefix=zzzz-no-match-{}",
            uuid::Uuid::new_v4()
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(body.is_empty());
}

#[tokio::test]
async fn list_schemas_subject_prefix_filters() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let uid = uuid::Uuid::new_v4();
    let s1 = format!("prefix-orders-{uid}");
    let s2 = format!("prefix-users-{uid}");

    common::api::register_schema(&client, &base, &s1, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &s2, common::AVRO_SCHEMA_V1).await;

    let resp = client
        .get(format!("{base}/schemas?subjectPrefix=prefix-orders-{uid}"))
        .send()
        .await
        .unwrap();
    let body: Vec<serde_json::Value> = resp.json().await.unwrap();

    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["subject"], s1);
}

#[tokio::test]
async fn list_schemas_latest_only() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let uid = uuid::Uuid::new_v4();
    let s1 = format!("latest-only-{uid}");

    common::api::register_schema(&client, &base, &s1, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &s1, common::AVRO_SCHEMA_V2).await;

    let resp = client
        .get(format!(
            "{base}/schemas?latestOnly=true&subjectPrefix=latest-only-{uid}"
        ))
        .send()
        .await
        .unwrap();
    let body: Vec<serde_json::Value> = resp.json().await.unwrap();

    assert_eq!(
        body.len(),
        1,
        "latestOnly should return 1 entry per subject"
    );
    assert_eq!(body[0]["version"], 2, "should return the latest version");
}

#[tokio::test]
async fn list_schemas_deleted_includes_soft_deleted() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let uid = uuid::Uuid::new_v4();
    let s1 = format!("del-list-{uid}");

    common::api::register_schema(&client, &base, &s1, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &s1, common::AVRO_SCHEMA_V2).await;
    common::api::delete_version(&client, &base, &s1, "2").await;

    // Without deleted → only version 1.
    let resp = client
        .get(format!("{base}/schemas?subjectPrefix=del-list-{uid}"))
        .send()
        .await
        .unwrap();
    let body: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["version"], 1);

    // With deleted=true → both versions.
    let resp = client
        .get(format!(
            "{base}/schemas?deleted=true&subjectPrefix=del-list-{uid}"
        ))
        .send()
        .await
        .unwrap();
    let body: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(body.len(), 2);
}

#[tokio::test]
async fn list_schemas_deleted_and_latest_only() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let uid = uuid::Uuid::new_v4();
    let s1 = format!("del-latest-{uid}");

    common::api::register_schema(&client, &base, &s1, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &s1, common::AVRO_SCHEMA_V2).await;
    common::api::delete_version(&client, &base, &s1, "2").await;

    // latestOnly + deleted=true → version 2 (soft-deleted, but it's the latest).
    let resp = client
        .get(format!(
            "{base}/schemas?deleted=true&latestOnly=true&subjectPrefix=del-latest-{uid}"
        ))
        .send()
        .await
        .unwrap();
    let body: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["version"], 2);
}

#[tokio::test]
async fn list_schemas_pagination() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let uid = uuid::Uuid::new_v4();
    let s1 = format!("page-{uid}");

    common::api::register_schema(&client, &base, &s1, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &s1, common::AVRO_SCHEMA_V2).await;
    common::api::register_schema(&client, &base, &s1, common::AVRO_SCHEMA_V3).await;

    // offset=1, limit=1 → should return exactly 1 result (version 2).
    let resp = client
        .get(format!(
            "{base}/schemas?subjectPrefix=page-{uid}&offset=1&limit=1"
        ))
        .send()
        .await
        .unwrap();
    let body: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["version"], 2);
}

#[tokio::test]
async fn list_schemas_omits_schema_type_for_avro() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let uid = uuid::Uuid::new_v4();
    let s_avro = format!("type-avro-{uid}");
    let s_json = format!("type-json-{uid}");

    common::api::register_schema(&client, &base, &s_avro, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema_with_type(&client, &base, &s_json, common::JSON_SCHEMA_V1, "JSON")
        .await;

    let resp = client
        .get(format!("{base}/schemas?subjectPrefix=type-"))
        .send()
        .await
        .unwrap();
    let body: Vec<serde_json::Value> = resp.json().await.unwrap();

    let avro_entry = body.iter().find(|v| v["subject"] == s_avro).unwrap();
    let json_entry = body.iter().find(|v| v["subject"] == s_json).unwrap();

    assert_eq!(avro_entry["schemaType"], "AVRO");
    assert_eq!(
        json_entry["schemaType"], "JSON",
        "schemaType should be present for JSON"
    );
}

#[tokio::test]
async fn list_schemas_like_metacharacter_escaping() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let uid = uuid::Uuid::new_v4();
    // Register with a subject containing LIKE metacharacters.
    let s1 = format!("meta_100%-{uid}");
    let s2 = format!("meta_nope-{uid}");

    common::api::register_schema(&client, &base, &s1, common::AVRO_SCHEMA_V1).await;
    common::api::register_schema(&client, &base, &s2, common::AVRO_SCHEMA_V1).await;

    // Prefix "meta_100%" should match s1 but not s2.
    let resp = client
        .get(format!("{base}/schemas?subjectPrefix=meta_100%25-{uid}"))
        .send()
        .await
        .unwrap();
    let body: Vec<serde_json::Value> = resp.json().await.unwrap();

    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["subject"], s1);
}
