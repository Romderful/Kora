//! Schema reference storage operations.

use sqlx::PgPool;

use crate::api::subjects::SchemaReference;
use crate::error::KoraError;

// -- Queries --

/// Validate that all referenced schemas exist and are not soft-deleted.
///
/// # Errors
///
/// Returns `KoraError::ReferenceNotFound` if any referenced subject/version
/// does not exist or is soft-deleted.
pub async fn validate_references(
    pool: &PgPool,
    refs: &[SchemaReference],
) -> Result<(), KoraError> {
    for r in refs {
        let exists = sqlx::query_scalar::<_, bool>(
            r"SELECT EXISTS(
                SELECT 1 FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
                WHERE sub.name = $1 AND s.version = $2
                  AND s.deleted = false AND sub.deleted = false
            )",
        )
        .bind(&r.subject)
        .bind(r.version)
        .fetch_one(pool)
        .await
        .map_err(|e| KoraError::BackendDataStore(e.to_string()))?;

        if !exists {
            return Err(KoraError::ReferenceNotFound(format!(
                "Schema reference not found: subject '{}' version {}",
                r.subject, r.version
            )));
        }
    }
    Ok(())
}

/// Insert schema references for a newly registered schema.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn insert_references(
    pool: &PgPool,
    schema_id: i64,
    refs: &[SchemaReference],
) -> Result<(), sqlx::Error> {
    for r in refs {
        sqlx::query(
            "INSERT INTO schema_references (schema_id, name, subject, version) VALUES ($1, $2, $3, $4)",
        )
        .bind(schema_id)
        .bind(&r.name)
        .bind(&r.subject)
        .bind(r.version)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Check if a subject/version is referenced by any **active** (non-deleted) schema.
///
/// Joins with the schemas table to ignore references from soft-deleted or
/// hard-deleted schemas — a deleted dependent should not block deletion of
/// its dependency.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn is_version_referenced(
    pool: &PgPool,
    subject: &str,
    version: i32,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        r"SELECT EXISTS(
            SELECT 1 FROM schema_references sr
            JOIN schemas s ON sr.schema_id = s.id
            WHERE sr.subject = $1 AND sr.version = $2
              AND s.deleted = false
        )",
    )
    .bind(subject)
    .bind(version)
    .fetch_one(pool)
    .await
}

