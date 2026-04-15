//! API call helpers for integration tests.

use reqwest::{Client, Response};

// -- Schema operations --

/// Register a schema under a subject and return its global ID.
pub async fn register_schema(client: &Client, base: &str, subject: &str, schema: &str) -> i64 {
    let resp = client
        .post(format!("{base}/subjects/{subject}/versions"))
        .json(&serde_json::json!({"schema": schema}))
        .send()
        .await
        .unwrap();
    resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap()
}

/// Register a schema with an explicit type under a subject and return its global ID.
pub async fn register_schema_with_type(
    client: &Client,
    base: &str,
    subject: &str,
    schema: &str,
    schema_type: &str,
) -> i64 {
    let resp = client
        .post(format!("{base}/subjects/{subject}/versions"))
        .json(&serde_json::json!({"schema": schema, "schemaType": schema_type}))
        .send()
        .await
        .unwrap();
    resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap()
}

/// Register a schema with references under a subject and return the raw response.
pub async fn register_schema_with_refs(
    client: &Client,
    base: &str,
    subject: &str,
    schema: &str,
    refs: &serde_json::Value,
) -> Response {
    client
        .post(format!("{base}/subjects/{subject}/versions"))
        .json(&serde_json::json!({
            "schema": schema,
            "references": refs
        }))
        .send()
        .await
        .unwrap()
}

/// Check if a schema is registered under a subject.
pub async fn check_schema(client: &Client, base: &str, subject: &str, schema: &str) -> Response {
    client
        .post(format!("{base}/subjects/{subject}"))
        .json(&serde_json::json!({"schema": schema}))
        .send()
        .await
        .unwrap()
}

/// Retrieve a schema by its global ID.
pub async fn get_schema_by_id(client: &Client, base: &str, id: i64) -> Response {
    client
        .get(format!("{base}/schemas/ids/{id}"))
        .send()
        .await
        .unwrap()
}

/// Retrieve a schema by subject and version (version can be a number or "latest").
pub async fn get_schema_by_version(
    client: &Client,
    base: &str,
    subject: &str,
    version: &str,
) -> Response {
    client
        .get(format!("{base}/subjects/{subject}/versions/{version}"))
        .send()
        .await
        .unwrap()
}

// -- List operations --

/// List subjects, optionally including soft-deleted ones.
pub async fn list_subjects(client: &Client, base: &str, include_deleted: bool) -> Vec<String> {
    let url = if include_deleted {
        format!("{base}/subjects?deleted=true")
    } else {
        format!("{base}/subjects")
    };
    client.get(url).send().await.unwrap().json().await.unwrap()
}

/// List versions for a subject, optionally including soft-deleted ones.
pub async fn list_versions(
    client: &Client,
    base: &str,
    subject: &str,
    include_deleted: bool,
) -> Vec<i32> {
    let url = if include_deleted {
        format!("{base}/subjects/{subject}/versions?deleted=true")
    } else {
        format!("{base}/subjects/{subject}/versions")
    };
    client.get(url).send().await.unwrap().json().await.unwrap()
}

// -- Delete operations --

/// Soft-delete a subject.
pub async fn delete_subject(client: &Client, base: &str, subject: &str) -> Response {
    client
        .delete(format!("{base}/subjects/{subject}"))
        .send()
        .await
        .unwrap()
}

/// Soft-delete a single schema version.
pub async fn delete_version(client: &Client, base: &str, subject: &str, version: &str) -> Response {
    client
        .delete(format!("{base}/subjects/{subject}/versions/{version}"))
        .send()
        .await
        .unwrap()
}

/// Hard-delete a subject (must be soft-deleted first).
pub async fn hard_delete_subject(client: &Client, base: &str, subject: &str) -> Response {
    client
        .delete(format!("{base}/subjects/{subject}?permanent=true"))
        .send()
        .await
        .unwrap()
}

/// Hard-delete a single schema version (must be soft-deleted first).
pub async fn hard_delete_version(
    client: &Client,
    base: &str,
    subject: &str,
    version: i32,
) -> Response {
    client
        .delete(format!(
            "{base}/subjects/{subject}/versions/{version}?permanent=true"
        ))
        .send()
        .await
        .unwrap()
}

// -- Cross-reference operations --

/// List subjects associated with a schema ID.
pub async fn get_subjects_by_schema_id(client: &Client, base: &str, id: i64) -> Response {
    client
        .get(format!("{base}/schemas/ids/{id}/subjects"))
        .send()
        .await
        .unwrap()
}

/// List subject-version pairs associated with a schema ID.
pub async fn get_versions_by_schema_id(client: &Client, base: &str, id: i64) -> Response {
    client
        .get(format!("{base}/schemas/ids/{id}/versions"))
        .send()
        .await
        .unwrap()
}

// -- Compatibility test operations --

/// Test schema compatibility against a specific version (or "latest").
pub async fn test_compatibility(
    client: &Client,
    base: &str,
    subject: &str,
    version: &str,
    schema: &str,
    schema_type: &str,
) -> Response {
    client
        .post(format!(
            "{base}/compatibility/subjects/{subject}/versions/{version}"
        ))
        .json(&serde_json::json!({"schema": schema, "schemaType": schema_type}))
        .send()
        .await
        .unwrap()
}

/// Test schema compatibility: register V1, check V2 against latest. Returns `is_compatible`.
pub async fn check_compatibility(
    client: &Client,
    base: &str,
    v1: &str,
    v2: &str,
    schema_type: &str,
) -> bool {
    let subject = format!("compat-{schema_type}-{}", uuid::Uuid::new_v4());
    register_schema_with_type(client, base, &subject, v1, schema_type).await;
    let resp = test_compatibility(client, base, &subject, "latest", v2, schema_type).await;
    resp.json::<serde_json::Value>().await.unwrap()["is_compatible"]
        .as_bool()
        .unwrap()
}

// -- Compatibility config operations --

/// Get the global compatibility level.
pub async fn get_global_compatibility(client: &Client, base: &str) -> Response {
    client.get(format!("{base}/config")).send().await.unwrap()
}

/// Set the global compatibility level.
pub async fn set_global_compatibility(
    client: &Client,
    base: &str,
    compatibility: &str,
) -> Response {
    client
        .put(format!("{base}/config"))
        .json(&serde_json::json!({"compatibility": compatibility}))
        .send()
        .await
        .unwrap()
}

/// Get the per-subject compatibility level.
pub async fn get_subject_compatibility(client: &Client, base: &str, subject: &str) -> Response {
    client
        .get(format!("{base}/config/{subject}"))
        .send()
        .await
        .unwrap()
}

/// Set the per-subject compatibility level.
pub async fn set_subject_compatibility(
    client: &Client,
    base: &str,
    subject: &str,
    compatibility: &str,
) -> Response {
    client
        .put(format!("{base}/config/{subject}"))
        .json(&serde_json::json!({"compatibility": compatibility}))
        .send()
        .await
        .unwrap()
}

/// Delete the per-subject compatibility level (falls back to global).
pub async fn delete_subject_compatibility(client: &Client, base: &str, subject: &str) -> Response {
    client
        .delete(format!("{base}/config/{subject}"))
        .send()
        .await
        .unwrap()
}

// -- Mode operations --

/// Get the global registry mode.
pub async fn get_global_mode(client: &Client, base: &str) -> Response {
    client.get(format!("{base}/mode")).send().await.unwrap()
}

/// Set the global registry mode.
pub async fn set_global_mode(client: &Client, base: &str, mode: &str) -> Response {
    client
        .put(format!("{base}/mode"))
        .json(&serde_json::json!({"mode": mode}))
        .send()
        .await
        .unwrap()
}

/// Delete (reset) the global registry mode to READWRITE.
pub async fn delete_global_mode(client: &Client, base: &str) -> Response {
    client.delete(format!("{base}/mode")).send().await.unwrap()
}

/// Get the per-subject registry mode.
pub async fn get_subject_mode(client: &Client, base: &str, subject: &str) -> Response {
    client
        .get(format!("{base}/mode/{subject}"))
        .send()
        .await
        .unwrap()
}

/// Set the per-subject registry mode.
pub async fn set_subject_mode(client: &Client, base: &str, subject: &str, mode: &str) -> Response {
    client
        .put(format!("{base}/mode/{subject}"))
        .json(&serde_json::json!({"mode": mode}))
        .send()
        .await
        .unwrap()
}

/// Delete the per-subject registry mode (falls back to global).
pub async fn delete_subject_mode(client: &Client, base: &str, subject: &str) -> Response {
    client
        .delete(format!("{base}/mode/{subject}"))
        .send()
        .await
        .unwrap()
}
