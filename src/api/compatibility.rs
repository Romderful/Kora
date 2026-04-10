//! Compatibility configuration API handlers.

use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};
use serde::Deserialize;
use sqlx::PgPool;

use crate::error::KoraError;
use crate::storage::{compatibility, subjects};

// -- Types --

/// Valid compatibility levels matching the Confluent Schema Registry spec.
pub const COMPATIBILITY_LEVELS: &[&str] = &[
    "BACKWARD",
    "BACKWARD_TRANSITIVE",
    "FORWARD",
    "FORWARD_TRANSITIVE",
    "FULL",
    "FULL_TRANSITIVE",
    "NONE",
];

/// Request body for compatibility config updates.
#[derive(Debug, Deserialize)]
pub struct CompatibilityRequest {
    /// The compatibility level to set.
    pub compatibility: String,
}

// -- Handlers --

/// Get the global compatibility configuration.
///
/// `GET /config`
///
/// # Errors
///
/// Returns `KoraError::BackendDataStore` (500) for database failures.
pub async fn get_global_compatibility(
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, KoraError> {
    let level = compatibility::get_global_level(&pool).await?;
    Ok(Json(serde_json::json!({ "compatibilityLevel": level })))
}

/// Update the global compatibility configuration.
///
/// `PUT /config`
///
/// # Errors
///
/// Returns `KoraError::InvalidCompatibilityLevel` (42203) for invalid levels.
pub async fn set_global_compatibility(
    State(pool): State<PgPool>,
    Json(body): Json<CompatibilityRequest>,
) -> Result<impl IntoResponse, KoraError> {
    validate_level(&body.compatibility)?;
    let level = compatibility::set_global_level(&pool, &body.compatibility).await?;
    Ok(Json(serde_json::json!({ "compatibility": level })))
}

/// Get the compatibility configuration for a subject.
///
/// `GET /config/{subject}`
///
/// Returns the per-subject level if set, otherwise the global fallback.
///
/// # Errors
///
/// Returns `KoraError::SubjectNotFound` (40401) if the subject doesn't exist.
pub async fn get_subject_compatibility(
    State(pool): State<PgPool>,
    Path(subject): Path<String>,
) -> Result<impl IntoResponse, KoraError> {
    if !subjects::subject_exists(&pool, &subject).await? {
        return Err(KoraError::SubjectNotFound);
    }

    let level = compatibility::get_level(&pool, &subject).await?;
    Ok(Json(serde_json::json!({ "compatibilityLevel": level })))
}

/// Update the compatibility configuration for a subject.
///
/// `PUT /config/{subject}`
///
/// # Errors
///
/// Returns `KoraError::SubjectNotFound` (40401) if the subject doesn't exist,
/// or `KoraError::InvalidCompatibilityLevel` (42203) for invalid levels.
pub async fn set_subject_compatibility(
    State(pool): State<PgPool>,
    Path(subject): Path<String>,
    Json(body): Json<CompatibilityRequest>,
) -> Result<impl IntoResponse, KoraError> {
    if !subjects::subject_exists(&pool, &subject).await? {
        return Err(KoraError::SubjectNotFound);
    }

    validate_level(&body.compatibility)?;
    let level = compatibility::set_subject_level(&pool, &subject, &body.compatibility).await?;
    Ok(Json(serde_json::json!({ "compatibility": level })))
}

/// Delete per-subject compatibility configuration (falls back to global).
///
/// `DELETE /config/{subject}`
///
/// # Errors
///
/// Returns `KoraError::SubjectNotFound` (40401) if the subject doesn't exist.
pub async fn delete_subject_compatibility(
    State(pool): State<PgPool>,
    Path(subject): Path<String>,
) -> Result<impl IntoResponse, KoraError> {
    if !subjects::subject_exists(&pool, &subject).await? {
        return Err(KoraError::SubjectNotFound);
    }

    let fallback = compatibility::delete_subject_level(&pool, &subject).await?;
    Ok(Json(serde_json::json!({ "compatibility": fallback })))
}

// -- Helpers --

/// Validate that a compatibility level string is one of the known values.
fn validate_level(level: &str) -> Result<(), KoraError> {
    if COMPATIBILITY_LEVELS.contains(&level) {
        Ok(())
    } else {
        Err(KoraError::InvalidCompatibilityLevel(level.to_string()))
    }
}
