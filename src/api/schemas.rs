//! Schema-related API handlers.

use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};
use sqlx::PgPool;

use crate::error::KoraError;
use crate::schema::SchemaFormat;
use crate::storage::schemas;

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
) -> Result<impl IntoResponse, KoraError> {
    let (schema_text, schema_type) = schemas::find_schema_by_id(&pool, id)
        .await?
        .ok_or(KoraError::SchemaNotFound)?;

    Ok(Json(serde_json::json!({
        "id": id,
        "schema": schema_text,
        "schemaType": schema_type,
    })))
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
) -> Result<impl IntoResponse, KoraError> {
    if !schemas::schema_exists(&pool, id).await? {
        return Err(KoraError::SchemaNotFound);
    }
    let subjects = schemas::find_subjects_by_schema_id(&pool, id).await?;
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
) -> Result<impl IntoResponse, KoraError> {
    if !schemas::schema_exists(&pool, id).await? {
        return Err(KoraError::SchemaNotFound);
    }
    let versions = schemas::find_versions_by_schema_id(&pool, id).await?;
    Ok(Json(versions))
}

/// List supported schema types.
///
/// `GET /schemas/types`
pub async fn list_schema_types() -> impl IntoResponse {
    Json(SchemaFormat::KNOWN_TYPES)
}
