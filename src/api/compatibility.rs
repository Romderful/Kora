//! Compatibility configuration API handlers.

use axum::{
    Json,
    extract::{Path, Query, State},
    response::IntoResponse,
};
use serde::Deserialize;
use sqlx::PgPool;

use crate::error::KoraError;
use crate::storage::compatibility;

/// Query parameter for `defaultToGlobal` on config GET endpoints.
#[derive(Debug, Deserialize)]
pub struct DefaultToGlobalParams {
    /// When true, fall back to global config if subject has no per-subject config.
    #[serde(default, rename = "defaultToGlobal")]
    pub default_to_global: bool,
}

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
    /// When true, normalize schemas before comparison (persisted per-subject or globally).
    #[serde(default)]
    pub normalize: bool,
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
    Query(_params): Query<DefaultToGlobalParams>,
) -> Result<impl IntoResponse, KoraError> {
    let level = compatibility::get_global_level(&pool).await?;
    let normalize = compatibility::get_global_normalize(&pool).await?;
    Ok(Json(serde_json::json!({ "compatibilityLevel": level, "normalize": normalize })))
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
    let level = compatibility::set_global_level(&pool, &body.compatibility, body.normalize).await?;
    Ok(Json(serde_json::json!({ "compatibility": level, "normalize": body.normalize })))
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
    Query(params): Query<DefaultToGlobalParams>,
) -> Result<impl IntoResponse, KoraError> {
    // Confluent does not check subject existence — only config existence.
    if let Some(level) = compatibility::get_subject_level(&pool, &subject).await? {
        let normalize = compatibility::get_subject_normalize(&pool, &subject).await?.unwrap_or(false);
        return Ok(Json(serde_json::json!({ "compatibilityLevel": level, "normalize": normalize })));
    }

    // No per-subject config — fall back or return 40408.
    if params.default_to_global {
        let level = compatibility::get_global_level(&pool).await?;
        let normalize = compatibility::get_global_normalize(&pool).await?;
        Ok(Json(serde_json::json!({ "compatibilityLevel": level, "normalize": normalize })))
    } else {
        Err(KoraError::SubjectCompatibilityNotConfigured(subject))
    }
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
    // Confluent allows setting config on any subject name (no existence check).
    validate_level(&body.compatibility)?;
    let level = compatibility::set_subject_level(&pool, &subject, &body.compatibility, body.normalize).await?;
    Ok(Json(serde_json::json!({ "compatibility": level, "normalize": body.normalize })))
}

/// Delete global compatibility configuration (reset to BACKWARD).
///
/// `DELETE /config`
///
/// # Errors
///
/// Returns `KoraError::BackendDataStore` (50001) for database failures.
pub async fn delete_global_compatibility(
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, KoraError> {
    let (prev_level, prev_normalize) = compatibility::delete_global_level(&pool).await?;
    Ok(Json(serde_json::json!({ "compatibilityLevel": prev_level, "normalize": prev_normalize })))
}

/// Delete per-subject compatibility configuration.
///
/// `DELETE /config/{subject}`
///
/// Returns the **previous** per-subject level (not the global fallback).
///
/// # Errors
///
/// Returns `KoraError::SubjectNotFound` (40401) if the subject doesn't exist.
pub async fn delete_subject_compatibility(
    State(pool): State<PgPool>,
    Path(subject): Path<String>,
) -> Result<impl IntoResponse, KoraError> {
    // Confluent checks config existence, not subject existence.
    let (prev_level, prev_normalize) = compatibility::delete_subject_level(&pool, &subject)
        .await?
        .ok_or(KoraError::SubjectNotFound)?;
    Ok(Json(serde_json::json!({ "compatibilityLevel": prev_level, "normalize": prev_normalize })))
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
