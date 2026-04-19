//! Integration tests for schema ID cross-references
//! (`GET /schemas/ids/{id}/subjects`, `GET /schemas/ids/{id}/versions`).

mod common;

use reqwest::StatusCode;

// -- GET /schemas/ids/{id}/subjects --

#[tokio::test]
async fn get_subjects_by_schema_id_succeeds() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("xref-subj-{}", uuid::Uuid::new_v4());
    let schema = common::unique_avro_schema();

    let id = common::api::register_schema(&client, &base, &subject, &schema).await;

    let resp = common::api::get_subjects_by_schema_id(&client, &base, id).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Vec<String> = resp.json().await.unwrap();
    assert_eq!(body, vec![subject]);
}

#[tokio::test]
async fn get_subjects_by_schema_id_nonexistent_returns_404() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = common::api::get_subjects_by_schema_id(&client, &base, i64::MAX).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40403);
}

#[tokio::test]
async fn get_subjects_by_schema_id_excludes_soft_deleted_subject() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("xref-sdel-{}", uuid::Uuid::new_v4());
    let schema = common::unique_avro_schema();

    let id = common::api::register_schema(&client, &base, &subject, &schema).await;
    common::api::delete_subject(&client, &base, &subject).await;

    let resp = common::api::get_subjects_by_schema_id(&client, &base, id).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Vec<String> = resp.json().await.unwrap();
    assert!(body.is_empty());
}

#[tokio::test]
async fn get_subjects_by_schema_id_excludes_soft_deleted_version() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("xref-vdel-{}", uuid::Uuid::new_v4());
    let schema = common::unique_avro_schema();

    let id = common::api::register_schema(&client, &base, &subject, &schema).await;
    common::api::delete_version(&client, &base, &subject, "1").await;

    let resp = common::api::get_subjects_by_schema_id(&client, &base, id).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Vec<String> = resp.json().await.unwrap();
    assert!(body.is_empty());
}

// -- GET /schemas/ids/{id}/versions --

#[tokio::test]
async fn get_versions_by_schema_id_succeeds() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("xref-ver-{}", uuid::Uuid::new_v4());
    let schema = common::unique_avro_schema();

    let id = common::api::register_schema(&client, &base, &subject, &schema).await;

    let resp = common::api::get_versions_by_schema_id(&client, &base, id).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["subject"], subject);
    assert_eq!(body[0]["version"], 1);
}

#[tokio::test]
async fn get_versions_by_schema_id_multiple_succeeds() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("xref-multi-{}", uuid::Uuid::new_v4());
    let schema1 = common::unique_avro_schema();
    let schema2 = common::unique_avro_schema();

    // Different record names are backward-incompatible — disable compat for this cross-ref test.
    common::api::set_subject_compatibility(&client, &base, &subject, "NONE").await;

    let id1 = common::api::register_schema(&client, &base, &subject, &schema1).await;
    let id2 = common::api::register_schema(&client, &base, &subject, &schema2).await;

    let resp = common::api::get_versions_by_schema_id(&client, &base, id1).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["subject"], subject);
    assert_eq!(body[0]["version"], 1);

    let resp2 = common::api::get_versions_by_schema_id(&client, &base, id2).await;
    assert_eq!(resp2.status(), StatusCode::OK);
    let body2: Vec<serde_json::Value> = resp2.json().await.unwrap();
    assert_eq!(body2.len(), 1);
    assert_eq!(body2[0]["subject"], subject);
    assert_eq!(body2[0]["version"], 2);
}

#[tokio::test]
async fn get_versions_by_schema_id_nonexistent_returns_404() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();

    let resp = common::api::get_versions_by_schema_id(&client, &base, i64::MAX).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error_code"], 40403);
}

#[tokio::test]
async fn get_versions_by_schema_id_excludes_soft_deleted_version() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("xref-verdel-{}", uuid::Uuid::new_v4());
    let schema = common::unique_avro_schema();

    let id = common::api::register_schema(&client, &base, &subject, &schema).await;
    common::api::delete_version(&client, &base, &subject, "1").await;

    let resp = common::api::get_versions_by_schema_id(&client, &base, id).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(body.is_empty());
}

#[tokio::test]
async fn get_versions_by_schema_id_excludes_soft_deleted_subject() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("xref-allempty-{}", uuid::Uuid::new_v4());
    let schema = common::unique_avro_schema();

    let id = common::api::register_schema(&client, &base, &subject, &schema).await;
    common::api::delete_subject(&client, &base, &subject).await;

    let resp = common::api::get_versions_by_schema_id(&client, &base, id).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(body.is_empty());
}

// -- Story 4.3: deleted + subject params --

#[tokio::test]
async fn get_subjects_by_schema_id_with_deleted_includes_soft_deleted() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("xref-sdel-{}", uuid::Uuid::new_v4());
    let schema = common::unique_avro_schema();

    let id = common::api::register_schema(&client, &base, &subject, &schema).await;
    common::api::delete_subject(&client, &base, &subject).await;

    let resp = common::api::get_subjects_by_schema_id(&client, &base, id).await;
    let subjects: Vec<String> = resp.json().await.unwrap();
    assert!(subjects.is_empty());

    let resp = client
        .get(format!("{base}/schemas/ids/{id}/subjects?deleted=true"))
        .send()
        .await
        .unwrap();
    let subjects: Vec<String> = resp.json().await.unwrap();
    assert_eq!(
        subjects,
        vec![subject],
        "deleted=true should include soft-deleted subject"
    );
}

#[tokio::test]
async fn get_subjects_by_schema_id_with_subject_filter() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("xref-filt-{}", uuid::Uuid::new_v4());
    let other = format!("xref-other-{}", uuid::Uuid::new_v4());
    let schema = common::unique_avro_schema();

    let id = common::api::register_schema(&client, &base, &subject, &schema).await;

    let resp = client
        .get(format!(
            "{base}/schemas/ids/{id}/subjects?subject={subject}"
        ))
        .send()
        .await
        .unwrap();
    let subjects: Vec<String> = resp.json().await.unwrap();
    assert_eq!(subjects, vec![subject.clone()]);

    let resp = client
        .get(format!("{base}/schemas/ids/{id}/subjects?subject={other}"))
        .send()
        .await
        .unwrap();
    let subjects: Vec<String> = resp.json().await.unwrap();
    assert!(
        subjects.is_empty(),
        "non-matching subject filter should return empty"
    );
}

#[tokio::test]
async fn get_versions_by_schema_id_with_deleted_includes_soft_deleted() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("xref-ver-del-{}", uuid::Uuid::new_v4());
    let schema = common::unique_avro_schema();

    let id = common::api::register_schema(&client, &base, &subject, &schema).await;
    common::api::delete_subject(&client, &base, &subject).await;

    let resp = common::api::get_versions_by_schema_id(&client, &base, id).await;
    let versions: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(versions.is_empty());

    let resp = client
        .get(format!("{base}/schemas/ids/{id}/versions?deleted=true"))
        .send()
        .await
        .unwrap();
    let versions: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(
        versions.len(),
        1,
        "deleted=true should include soft-deleted versions"
    );
    assert_eq!(versions[0]["subject"], subject);
    assert_eq!(versions[0]["version"], 1);
}

#[tokio::test]
async fn get_versions_by_schema_id_with_subject_filter() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("xref-vfilt-{}", uuid::Uuid::new_v4());
    let other = format!("xref-vother-{}", uuid::Uuid::new_v4());
    let schema = common::unique_avro_schema();

    let id = common::api::register_schema(&client, &base, &subject, &schema).await;

    let resp = client
        .get(format!(
            "{base}/schemas/ids/{id}/versions?subject={subject}"
        ))
        .send()
        .await
        .unwrap();
    let versions: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0]["subject"], subject);

    let resp = client
        .get(format!("{base}/schemas/ids/{id}/versions?subject={other}"))
        .send()
        .await
        .unwrap();
    let versions: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(
        versions.is_empty(),
        "non-matching subject filter should return empty"
    );
}

// -- Global dedup: same content under multiple subjects --

#[tokio::test]
async fn same_schema_different_subjects_returns_same_id() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let s1 = format!("dedup-a-{}", uuid::Uuid::new_v4());
    let s2 = format!("dedup-b-{}", uuid::Uuid::new_v4());
    let schema = common::unique_avro_schema();

    let id1 = common::api::register_schema(&client, &base, &s1, &schema).await;
    let id2 = common::api::register_schema(&client, &base, &s2, &schema).await;

    assert_eq!(
        id1, id2,
        "identical content under different subjects should share the same global ID"
    );
}

#[tokio::test]
async fn cross_ref_subjects_returns_multiple_subjects() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let s1 = format!("xref-multi-a-{}", uuid::Uuid::new_v4());
    let s2 = format!("xref-multi-b-{}", uuid::Uuid::new_v4());
    let schema = common::unique_avro_schema();

    let id = common::api::register_schema(&client, &base, &s1, &schema).await;
    common::api::register_schema(&client, &base, &s2, &schema).await;

    let resp = common::api::get_subjects_by_schema_id(&client, &base, id).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let mut subjects: Vec<String> = resp.json().await.unwrap();
    subjects.sort();
    let mut expected = vec![s1, s2];
    expected.sort();
    assert_eq!(subjects, expected, "cross-ref should return both subjects");
}

#[tokio::test]
async fn cross_ref_versions_returns_multiple_subject_versions() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let s1 = format!("xref-ver-multi-a-{}", uuid::Uuid::new_v4());
    let s2 = format!("xref-ver-multi-b-{}", uuid::Uuid::new_v4());
    let schema = common::unique_avro_schema();

    let id = common::api::register_schema(&client, &base, &s1, &schema).await;
    common::api::register_schema(&client, &base, &s2, &schema).await;

    let resp = common::api::get_versions_by_schema_id(&client, &base, id).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let versions: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(
        versions.len(),
        2,
        "cross-ref should return version entries for both subjects"
    );
}

#[tokio::test]
async fn cross_ref_soft_delete_one_subject_still_shows_other() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let s1 = format!("xref-sdel-a-{}", uuid::Uuid::new_v4());
    let s2 = format!("xref-sdel-b-{}", uuid::Uuid::new_v4());
    let schema = common::unique_avro_schema();

    let id = common::api::register_schema(&client, &base, &s1, &schema).await;
    common::api::register_schema(&client, &base, &s2, &schema).await;

    common::api::delete_subject(&client, &base, &s1).await;

    let resp = common::api::get_subjects_by_schema_id(&client, &base, id).await;
    let subjects: Vec<String> = resp.json().await.unwrap();
    assert_eq!(subjects, vec![s2.clone()]);

    let resp = client
        .get(format!("{base}/schemas/ids/{id}/subjects?deleted=true"))
        .send()
        .await
        .unwrap();
    let mut subjects: Vec<String> = resp.json().await.unwrap();
    subjects.sort();
    assert_eq!(subjects.len(), 2, "deleted=true should show both subjects");
}

#[tokio::test]
async fn content_survives_version_hard_delete() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let s1 = format!("xref-hdel-a-{}", uuid::Uuid::new_v4());
    let s2 = format!("xref-hdel-b-{}", uuid::Uuid::new_v4());
    let schema = common::unique_avro_schema();

    let id = common::api::register_schema(&client, &base, &s1, &schema).await;
    common::api::register_schema(&client, &base, &s2, &schema).await;

    common::api::delete_subject(&client, &base, &s1).await;
    common::api::hard_delete_subject(&client, &base, &s1).await;

    let resp = common::api::get_schema_by_id(&client, &base, id).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "content should survive hard-delete of one subject"
    );

    let resp = common::api::get_subjects_by_schema_id(&client, &base, id).await;
    let subjects: Vec<String> = resp.json().await.unwrap();
    assert_eq!(subjects, vec![s2]);
}

// -- Python-style booleans (case-insensitive query params) --

#[tokio::test]
async fn get_subjects_by_schema_id_accepts_python_style_deleted_true() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("py-xref-subj-{}", uuid::Uuid::new_v4());
    let schema = common::unique_avro_schema();

    let id = common::api::register_schema(&client, &base, &subject, &schema).await;
    common::api::delete_subject(&client, &base, &subject).await;

    // Python's str(True) → "True"
    let resp = client
        .get(format!("{base}/schemas/ids/{id}/subjects?deleted=True"))
        .send()
        .await
        .unwrap();
    let subjects: Vec<String> = resp.json().await.unwrap();
    assert_eq!(subjects, vec![subject]);
}

#[tokio::test]
async fn get_versions_by_schema_id_accepts_python_style_deleted_true() {
    let base = common::spawn_server().await;
    let client = reqwest::Client::new();
    let subject = format!("py-xref-ver-{}", uuid::Uuid::new_v4());
    let schema = common::unique_avro_schema();

    let id = common::api::register_schema(&client, &base, &subject, &schema).await;
    common::api::delete_subject(&client, &base, &subject).await;

    // Python's str(True) → "True"
    let resp = client
        .get(format!("{base}/schemas/ids/{id}/versions?deleted=True"))
        .send()
        .await
        .unwrap();
    let versions: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0]["subject"], subject);
}
