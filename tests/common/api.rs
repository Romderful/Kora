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
pub async fn get_schema_by_version(client: &Client, base: &str, subject: &str, version: &str) -> Response {
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
pub async fn list_versions(client: &Client, base: &str, subject: &str, include_deleted: bool) -> Vec<i32> {
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
pub async fn hard_delete_version(client: &Client, base: &str, subject: &str, version: i32) -> Response {
    client
        .delete(format!("{base}/subjects/{subject}/versions/{version}?permanent=true"))
        .send()
        .await
        .unwrap()
}
