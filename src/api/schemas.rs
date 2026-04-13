//! Schema-related API handlers.

use axum::{
    Json,
    extract::{Path, Query, State},
    response::IntoResponse,
};
use serde::Deserialize;
use sqlx::PgPool;

use crate::error::KoraError;
use crate::schema::SchemaFormat;
use crate::storage::{references, schemas};

/// Query parameters for cross-reference endpoints.
#[derive(Debug, Deserialize)]
pub struct CrossRefParams {
    /// When true, include soft-deleted subjects/versions in results.
    #[serde(default)]
    pub deleted: bool,
    /// Filter results to a specific subject name.
    #[serde(default)]
    pub subject: Option<String>,
    /// Pagination offset (default 0).
    #[serde(default)]
    pub offset: i64,
    /// Pagination limit (-1 = unlimited, default).
    #[serde(default = "default_limit")]
    pub limit: i64,
}

/// Query parameters for `GET /schemas/ids/{id}`.
#[derive(Debug, Deserialize)]
pub struct GetSchemaByIdParams {
    /// When true, include `maxId` in response.
    #[serde(default, rename = "fetchMaxId")]
    pub fetch_max_id: bool,
    /// Subject context (accept and ignore in MVP).
    #[serde(default)]
    pub subject: Option<String>,
    /// Output format (accept and ignore in MVP).
    #[serde(default)]
    pub format: String,
    /// Reference output format (accept and ignore in MVP).
    #[serde(default, rename = "referenceFormat")]
    pub reference_format: String,
}

/// Query parameters for `GET /schemas` (list all schemas).
#[derive(Debug, Deserialize)]
pub struct ListSchemasParams {
    /// When true, include soft-deleted versions/subjects.
    #[serde(default)]
    pub deleted: bool,
    /// When true, return only the highest version per subject.
    #[serde(default, rename = "latestOnly")]
    pub latest_only: bool,
    /// Subject name prefix filter. Default `:*:` means match all.
    #[serde(default = "default_subject_prefix", rename = "subjectPrefix")]
    pub subject_prefix: String,
    /// Pagination offset (default 0).
    #[serde(default)]
    pub offset: i64,
    /// Pagination limit (-1 = unlimited, default).
    #[serde(default = "default_limit")]
    pub limit: i64,
}

/// Query parameters for `GET /schemas/ids/{id}/schema` (schema text).
#[derive(Debug, Deserialize)]
pub struct GetSchemaTextParams {
    /// Subject context (accept and ignore — future: schema resolution context).
    #[serde(default)]
    pub subject: Option<String>,
}

use super::subjects::{default_limit, default_subject_prefix};

// -- Handlers --

/// Retrieve a schema by its global ID.
///
/// `GET /schemas/ids/{id}`
///
/// # Errors
///
/// Returns `KoraError::SchemaNotFound` (404) if no schema exists with the
/// given ID, or `KoraError::BackendDataStore` (500) for database failures.
pub async fn get_schema_by_id(
    State(pool): State<PgPool>,
    Path(id): Path<i64>,
    Query(params): Query<GetSchemaByIdParams>,
) -> Result<impl IntoResponse, KoraError> {
    let (schema_text, schema_type) = schemas::find_schema_by_id(&pool, id)
        .await?
        .ok_or(KoraError::SchemaNotFound)?;

    let refs = references::find_references_by_schema_id(&pool, id).await?;

    let mut body = serde_json::json!({
        "schema": schema_text,
        "id": id,
        "references": refs,
    });

    // Omit schemaType for AVRO (Confluent default behavior).
    if schema_type != "AVRO" {
        body["schemaType"] = serde_json::Value::String(schema_type);
    }

    if params.fetch_max_id {
        let max_id = schemas::find_max_schema_id(&pool).await?;
        body["maxId"] = serde_json::json!(max_id);
    }

    Ok(Json(body))
}

/// List subjects associated with a schema ID.
///
/// `GET /schemas/ids/{id}/subjects`
///
/// # Errors
///
/// Returns `KoraError::SchemaNotFound` (404) if no schema exists with the
/// given ID, or `KoraError::BackendDataStore` (500) for database failures.
pub async fn get_subjects_by_schema_id(
    State(pool): State<PgPool>,
    Path(id): Path<i64>,
    Query(params): Query<CrossRefParams>,
) -> Result<impl IntoResponse, KoraError> {
    if !schemas::schema_exists(&pool, id).await? {
        return Err(KoraError::SchemaNotFound);
    }
    let subjects = schemas::find_subjects_by_schema_id(
        &pool,
        id,
        params.deleted,
        params.subject.as_deref(),
        params.offset.max(0),
        params.limit,
    )
    .await?;
    Ok(Json(subjects))
}

/// List subject-version pairs associated with a schema ID.
///
/// `GET /schemas/ids/{id}/versions`
///
/// # Errors
///
/// Returns `KoraError::SchemaNotFound` (404) if no schema exists with the
/// given ID, or `KoraError::BackendDataStore` (500) for database failures.
pub async fn get_versions_by_schema_id(
    State(pool): State<PgPool>,
    Path(id): Path<i64>,
    Query(params): Query<CrossRefParams>,
) -> Result<impl IntoResponse, KoraError> {
    if !schemas::schema_exists(&pool, id).await? {
        return Err(KoraError::SchemaNotFound);
    }
    let versions = schemas::find_versions_by_schema_id(
        &pool,
        id,
        params.deleted,
        params.subject.as_deref(),
        params.offset.max(0),
        params.limit,
    )
    .await?;
    Ok(Json(versions))
}

/// List all schemas across all subjects, with optional filtering.
///
/// `GET /schemas`
///
/// # Errors
///
/// Returns `KoraError::BackendDataStore` (500) for database failures.
pub async fn list_schemas(
    State(pool): State<PgPool>,
    Query(params): Query<ListSchemasParams>,
) -> Result<impl IntoResponse, KoraError> {
    let prefix = if params.subject_prefix == ":*:" || params.subject_prefix.is_empty() {
        None
    } else {
        Some(params.subject_prefix.as_str())
    };
    let mut all = schemas::list_schemas(
        &pool,
        params.deleted,
        params.latest_only,
        prefix,
        params.offset.max(0),
        params.limit,
    )
    .await?;

    for sv in &mut all {
        sv.references = references::find_references_by_schema_id(&pool, sv.id).await?;
    }

    Ok(Json(all))
}

/// Retrieve schema text by global ID.
///
/// `GET /schemas/ids/{id}/schema`
///
/// # Errors
///
/// Returns `KoraError::SchemaNotFound` (40403) if no schema exists with the
/// given ID, or `KoraError::BackendDataStore` (500) for database failures.
pub async fn get_schema_text_by_id(
    State(pool): State<PgPool>,
    Path(id): Path<i64>,
    Query(_params): Query<GetSchemaTextParams>,
) -> Result<impl IntoResponse, KoraError> {
    let (schema_text, _schema_type) = schemas::find_schema_by_id(&pool, id)
        .await?
        .ok_or(KoraError::SchemaNotFound)?;
    Ok(Json(schema_text))
}

/// List supported schema types.
///
/// `GET /schemas/types`
pub async fn list_schema_types() -> impl IntoResponse {
    Json(SchemaFormat::KNOWN_TYPES)
}
