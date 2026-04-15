//! Subject storage operations.

use sqlx::PgPool;

// -- Queries --

/// List subject names, sorted alphabetically, with pagination.
///
/// - `include_deleted`: when true, include soft-deleted subjects in results.
/// - `deleted_only`: when true, return ONLY soft-deleted subjects (takes precedence).
/// - `prefix`: when `Some`, filter to subjects whose name starts with this prefix.
/// - `offset` defaults to 0, `limit` of -1 means unlimited.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn list_subjects(
    pool: &PgPool,
    include_deleted: bool,
    deleted_only: bool,
    prefix: Option<&str>,
    offset: i64,
    limit: i64,
) -> Result<Vec<String>, sqlx::Error> {
    let like_pattern = prefix.filter(|p| !p.is_empty()).map(|p| {
        let escaped = p
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        format!("{escaped}%")
    });

    if deleted_only {
        match (&like_pattern, limit >= 0) {
            (Some(pat), true) => sqlx::query_scalar(
                "SELECT name FROM subjects WHERE deleted = true AND name LIKE $1 ESCAPE '\\' ORDER BY name OFFSET $2 LIMIT $3",
            )
            .bind(pat)
            .bind(offset)
            .bind(limit)
            .fetch_all(pool)
            .await,

            (Some(pat), false) => sqlx::query_scalar(
                "SELECT name FROM subjects WHERE deleted = true AND name LIKE $1 ESCAPE '\\' ORDER BY name OFFSET $2",
            )
            .bind(pat)
            .bind(offset)
            .fetch_all(pool)
            .await,

            (None, true) => sqlx::query_scalar(
                "SELECT name FROM subjects WHERE deleted = true ORDER BY name OFFSET $1 LIMIT $2",
            )
            .bind(offset)
            .bind(limit)
            .fetch_all(pool)
            .await,

            (None, false) => sqlx::query_scalar(
                "SELECT name FROM subjects WHERE deleted = true ORDER BY name OFFSET $1",
            )
            .bind(offset)
            .fetch_all(pool)
            .await,
        }
    } else {
        match (&like_pattern, limit >= 0) {
            (Some(pat), true) => sqlx::query_scalar(
                "SELECT name FROM subjects WHERE (deleted = false OR $1) AND name LIKE $2 ESCAPE '\\' ORDER BY name OFFSET $3 LIMIT $4",
            )
            .bind(include_deleted)
            .bind(pat)
            .bind(offset)
            .bind(limit)
            .fetch_all(pool)
            .await,

            (Some(pat), false) => sqlx::query_scalar(
                "SELECT name FROM subjects WHERE (deleted = false OR $1) AND name LIKE $2 ESCAPE '\\' ORDER BY name OFFSET $3",
            )
            .bind(include_deleted)
            .bind(pat)
            .bind(offset)
            .fetch_all(pool)
            .await,

            (None, true) => sqlx::query_scalar(
                "SELECT name FROM subjects WHERE deleted = false OR $1 ORDER BY name OFFSET $2 LIMIT $3",
            )
            .bind(include_deleted)
            .bind(offset)
            .bind(limit)
            .fetch_all(pool)
            .await,

            (None, false) => sqlx::query_scalar(
                "SELECT name FROM subjects WHERE deleted = false OR $1 ORDER BY name OFFSET $2",
            )
            .bind(include_deleted)
            .bind(offset)
            .fetch_all(pool)
            .await,
        }
    }
}

/// Soft-delete a subject and all its schema versions. Returns the deleted version
/// numbers sorted ascending. Runs in a transaction for consistency.
///
/// # Errors
///
/// Returns a database error on connection or transaction failure.
pub async fn soft_delete_subject(pool: &PgPool, name: &str) -> Result<Vec<i32>, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let mut versions = sqlx::query_scalar::<_, i32>(
        r"UPDATE schema_versions SET deleted = true
           WHERE subject_id = (SELECT id FROM subjects WHERE name = $1) AND deleted = false
           RETURNING version",
    )
    .bind(name)
    .fetch_all(&mut *tx)
    .await?;

    sqlx::query("UPDATE subjects SET deleted = true WHERE name = $1 AND deleted = false")
        .bind(name)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    versions.sort_unstable();
    Ok(versions)
}

/// Hard-delete a soft-deleted subject and all its schemas. Returns the deleted
/// version numbers sorted ascending. Runs in a transaction.
///
/// Only operates on rows where `deleted = true` (must be soft-deleted first).
///
/// # Errors
///
/// Returns a database error on connection or transaction failure.
pub async fn hard_delete_subject(pool: &PgPool, name: &str) -> Result<Vec<i32>, sqlx::Error> {
    let mut tx = pool.begin().await?;

    // No need to clean up schema_references — they belong to schema_contents
    // which is never deleted (global IDs are permanent).
    let mut versions = sqlx::query_scalar::<_, i32>(
        r"DELETE FROM schema_versions
           WHERE subject_id = (SELECT id FROM subjects WHERE name = $1) AND deleted = true
           RETURNING version",
    )
    .bind(name)
    .fetch_all(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM subjects WHERE name = $1 AND deleted = true")
        .bind(name)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    versions.sort_unstable();
    Ok(versions)
}

/// Find a subject's ID by name, optionally including soft-deleted subjects.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn find_subject_id_by_name(
    pool: &PgPool,
    name: &str,
    include_deleted: bool,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT id FROM subjects WHERE name = $1 AND (deleted = false OR $2)",
    )
    .bind(name)
    .bind(include_deleted)
    .fetch_optional(pool)
    .await
}

/// Check if a subject exists by name, optionally including soft-deleted subjects.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn subject_exists(
    pool: &PgPool,
    name: &str,
    include_deleted: bool,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM subjects WHERE name = $1 AND (deleted = false OR $2))",
    )
    .bind(name)
    .bind(include_deleted)
    .fetch_one(pool)
    .await
}

/// Check if a subject is soft-deleted.
///
/// Returns true if subject exists AND is soft-deleted.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn subject_is_soft_deleted(pool: &PgPool, name: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM subjects WHERE name = $1 AND deleted = true)",
    )
    .bind(name)
    .fetch_one(pool)
    .await
}
