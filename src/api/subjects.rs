//! Subject-related API handlers.

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::error::KoraError;
use crate::schema::{self, SchemaFormat};
use crate::storage::{schemas, subjects};

/// Request body for `POST /subjects/{subject}/versions`.
#[derive(Debug, Deserialize)]
pub struct RegisterSchemaRequest {
    /// The raw schema string (JSON-encoded).
    pub schema: String,
    /// Schema format — defaults to AVRO when absent.
    #[serde(rename = "schemaType")]
    pub schema_type: Option<String>,
}

/// Response body for successful schema registration.
#[derive(Debug, Serialize)]
pub struct RegisterSchemaResponse {
    /// Globally unique sequential schema ID.
    pub id: i64,
}

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
    body: Result<Json<RegisterSchemaRequest>, JsonRejection>,
) -> Result<impl IntoResponse, KoraError> {
    let Json(body) = body.map_err(|e| KoraError::InvalidSchema(e.body_text()))?;

    validate_subject(&subject)?;

    let format = SchemaFormat::from_optional(body.schema_type.as_deref())?;
    let parsed = schema::parse(format, &body.schema)?;

    let subject_id = subjects::upsert(&pool, &subject).await?;

    // Idempotency: return existing ID if same schema already registered.
    if let Some(id) = schemas::find_by_fingerprint(&pool, subject_id, &parsed.fingerprint).await? {
        return Ok(Json(RegisterSchemaResponse { id }));
    }

    let id = schemas::insert(&pool, &schemas::NewSchema {
        subject_id,
        schema_type: format.as_str(),
        schema_text: &body.schema,
        canonical_form: &parsed.canonical_form,
        fingerprint: &parsed.fingerprint,
    })
    .await?;

    Ok(Json(RegisterSchemaResponse { id }))
}

/// List all registered subjects.
///
/// `GET /subjects`
///
/// # Errors
///
/// Returns `KoraError::BackendDataStore` (500) for database failures.
pub async fn list_subjects(
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, KoraError> {
    let names = subjects::list(&pool).await?;
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
) -> Result<impl IntoResponse, KoraError> {
    validate_subject(&subject)?;

    if !subjects::exists(&pool, &subject).await? {
        return Err(KoraError::SubjectNotFound);
    }

    let versions = schemas::list_versions(&pool, &subject).await?;
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
        let v: i32 = version.parse().map_err(|_| KoraError::VersionNotFound)?;
        if v < 1 {
            return Err(KoraError::VersionNotFound);
        }
        schemas::find_by_subject_version(&pool, &subject, v).await?
    };

    match row {
        Some(sv) => Ok(Json(sv)),
        None if subjects::exists(&pool, &subject).await? => Err(KoraError::VersionNotFound),
        None => Err(KoraError::SubjectNotFound),
    }
}

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
