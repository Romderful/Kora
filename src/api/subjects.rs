//! Subject-related API handlers.

use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection},
    response::IntoResponse,
};
use serde::Deserialize;
use sqlx::PgPool;

use crate::error::KoraError;
use crate::schema::{self, SchemaFormat};
use crate::storage::{schemas, subjects};

// -- Types --

/// Request body for schema registration and check endpoints.
#[derive(Debug, Deserialize)]
pub struct SchemaRequest {
    /// The raw schema string (JSON-encoded).
    pub schema: String,
    /// Schema format — defaults to AVRO when absent.
    #[serde(rename = "schemaType")]
    pub schema_type: Option<String>,
}

/// Query parameters for list endpoints supporting `?deleted=true`.
#[derive(Debug, Deserialize)]
pub struct DeletedParam {
    /// When true, include soft-deleted items in the response.
    #[serde(default)]
    pub deleted: bool,
}

/// Query parameters for DELETE endpoints supporting `?permanent=true`.
#[derive(Debug, Deserialize)]
pub struct PermanentParam {
    /// When true, hard-delete (requires prior soft-delete).
    #[serde(default)]
    pub permanent: bool,
}

// -- Handlers --

/// Register a schema under a subject.
///
/// `POST /subjects/{subject}/versions`
///
/// # Errors
///
/// Returns `KoraError::InvalidSchema` (422) for unparseable schemas or
/// `KoraError::BackendDataStore` (500) for database failures.
pub async fn register_schema(
    State(pool): State<PgPool>,
    Path(subject): Path<String>,
    body: Result<Json<SchemaRequest>, JsonRejection>,
) -> Result<impl IntoResponse, KoraError> {
    let Json(body) = body.map_err(|e| KoraError::InvalidSchema(e.body_text()))?;

    validate_subject(&subject)?;

    let format = SchemaFormat::from_optional(body.schema_type.as_deref())?;
    let parsed = schema::parse(format, &body.schema)?;

    let subject_id = subjects::upsert(&pool, &subject).await?;

    // Idempotency: return existing ID if same schema already registered.
    if let Some(id) = schemas::find_by_fingerprint(&pool, subject_id, &parsed.fingerprint).await? {
        return Ok(Json(serde_json::json!({ "id": id })));
    }

    let id = schemas::insert(&pool, &schemas::NewSchema {
        subject_id,
        schema_type: format.as_str(),
        schema_text: &body.schema,
        canonical_form: &parsed.canonical_form,
        fingerprint: &parsed.fingerprint,
    })
    .await?;

    Ok(Json(serde_json::json!({ "id": id })))
}

/// Check if a schema is registered under a subject.
///
/// `POST /subjects/{subject}`
///
/// # Errors
///
/// Returns `KoraError::SubjectNotFound` (40401) if the subject doesn't exist,
/// or `KoraError::SchemaNotFound` (40403) if the schema is not registered.
pub async fn check_schema(
    State(pool): State<PgPool>,
    Path(subject): Path<String>,
    body: Result<Json<SchemaRequest>, JsonRejection>,
) -> Result<impl IntoResponse, KoraError> {
    let Json(body) = body.map_err(|e| KoraError::InvalidSchema(e.body_text()))?;

    validate_subject(&subject)?;

    let format = SchemaFormat::from_optional(body.schema_type.as_deref())?;
    let parsed = schema::parse(format, &body.schema)?;

    if !subjects::exists(&pool, &subject).await? {
        return Err(KoraError::SubjectNotFound);
    }

    let sv = schemas::find_by_subject_fingerprint(&pool, &subject, &parsed.fingerprint)
        .await?
        .ok_or(KoraError::SchemaNotFound)?;

    Ok(Json(sv))
}

/// List registered subjects.
///
/// `GET /subjects` — non-deleted subjects (default).
/// `GET /subjects?deleted=true` — all subjects (including soft-deleted).
///
/// # Errors
///
/// Returns `KoraError::BackendDataStore` (500) for database failures.
pub async fn list_subjects(
    State(pool): State<PgPool>,
    Query(params): Query<DeletedParam>,
) -> Result<impl IntoResponse, KoraError> {
    let names = subjects::list(&pool, params.deleted).await?;
    Ok(Json(names))
}

/// List all versions of a subject.
///
/// `GET /subjects/{subject}/versions`
///
/// # Errors
///
/// Returns `KoraError::SubjectNotFound` (40401) if the subject doesn't exist,
/// or `KoraError::BackendDataStore` (500) for database failures.
pub async fn list_versions(
    State(pool): State<PgPool>,
    Path(subject): Path<String>,
    Query(params): Query<DeletedParam>,
) -> Result<impl IntoResponse, KoraError> {
    validate_subject(&subject)?;

    if !subjects::exists(&pool, &subject).await? {
        return Err(KoraError::SubjectNotFound);
    }

    let versions = schemas::list_versions(&pool, &subject, params.deleted).await?;
    Ok(Json(versions))
}

/// Retrieve a schema by subject and version.
///
/// `GET /subjects/{subject}/versions/{version}`
///
/// The version can be a number or "latest".
///
/// # Errors
///
/// Returns `KoraError::SubjectNotFound` (40401) if the subject doesn't exist,
/// or `KoraError::VersionNotFound` (40402) if the version doesn't exist.
pub async fn get_schema_by_version(
    State(pool): State<PgPool>,
    Path((subject, version)): Path<(String, String)>,
) -> Result<impl IntoResponse, KoraError> {
    validate_subject(&subject)?;

    let row = if version == "latest" {
        schemas::find_latest_by_subject(&pool, &subject).await?
    } else {
        let v = parse_version(&version)?;
        schemas::find_by_subject_version(&pool, &subject, v).await?
    };

    match row {
        Some(sv) => Ok(Json(sv)),
        None if subjects::exists(&pool, &subject).await? => Err(KoraError::VersionNotFound),
        None => Err(KoraError::SubjectNotFound),
    }
}

/// Soft-delete a subject and all its versions.
///
/// `DELETE /subjects/{subject}`
///
/// # Errors
///
/// Returns `KoraError::SubjectNotFound` (40401) if the subject doesn't exist
/// (or isn't soft-deleted when `permanent=true`).
pub async fn delete_subject(
    State(pool): State<PgPool>,
    Path(subject): Path<String>,
    Query(params): Query<PermanentParam>,
) -> Result<impl IntoResponse, KoraError> {
    validate_subject(&subject)?;

    if params.permanent {
        let versions = subjects::hard_delete(&pool, &subject).await?;
        if versions.is_empty() {
            return Err(KoraError::SubjectNotFound);
        }
        Ok(Json(versions))
    } else {
        if !subjects::exists(&pool, &subject).await? {
            return Err(KoraError::SubjectNotFound);
        }
        let versions = subjects::soft_delete(&pool, &subject).await?;
        Ok(Json(versions))
    }
}

/// Delete a single schema version (soft or hard).
///
/// `DELETE /subjects/{subject}/versions/{version}`
/// `DELETE /subjects/{subject}/versions/{version}?permanent=true`
///
/// # Errors
///
/// Returns `KoraError::SubjectNotFound` (40401) or `KoraError::VersionNotFound` (40402).
pub async fn delete_version(
    State(pool): State<PgPool>,
    Path((subject, version)): Path<(String, String)>,
    Query(params): Query<PermanentParam>,
) -> Result<impl IntoResponse, KoraError> {
    validate_subject(&subject)?;

    let deleted = if params.permanent {
        let v = parse_version(&version)?;
        schemas::hard_delete_version(&pool, &subject, v).await?
    } else {
        if !subjects::exists(&pool, &subject).await? {
            return Err(KoraError::SubjectNotFound);
        }
        if version == "latest" {
            schemas::soft_delete_latest(&pool, &subject).await?
        } else {
            let v = parse_version(&version)?;
            schemas::soft_delete_version(&pool, &subject, v).await?
        }
    }
    .ok_or(KoraError::VersionNotFound)?;

    Ok(Json(deleted))
}

// -- Helpers --

/// Maximum allowed length for a subject name.
const MAX_SUBJECT_LENGTH: usize = 255;

/// Validate the subject path parameter.
fn validate_subject(subject: &str) -> Result<(), KoraError> {
    if subject.is_empty() {
        return Err(KoraError::InvalidSchema(
            "Subject name must not be empty".into(),
        ));
    }
    if subject.len() > MAX_SUBJECT_LENGTH {
        return Err(KoraError::InvalidSchema(
            "Subject name exceeds maximum length".into(),
        ));
    }
    if subject.contains('\0') {
        return Err(KoraError::InvalidSchema(
            "Subject name contains invalid characters".into(),
        ));
    }
    Ok(())
}

/// Parse a version string to a positive i32.
fn parse_version(version: &str) -> Result<i32, KoraError> {
    let v: i32 = version.parse().map_err(|_| KoraError::VersionNotFound)?;
    if v < 1 {
        return Err(KoraError::VersionNotFound);
    }
    Ok(v)
}
