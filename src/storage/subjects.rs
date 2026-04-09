//! Subject storage operations.

use sqlx::PgPool;

// -- Queries --

/// Insert a subject if it doesn't exist and return its ID.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn upsert(pool: &PgPool, name: &str) -> Result<i64, sqlx::Error> {
    let id = sqlx::query_scalar::<_, i64>(
        r"WITH ins AS (
             INSERT INTO subjects (name) VALUES ($1)
             ON CONFLICT (name) DO NOTHING
             RETURNING id
           )
           SELECT id FROM ins
           UNION ALL
           SELECT id FROM subjects WHERE name = $1
           LIMIT 1",
    )
    .bind(name)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

/// List subject names, sorted alphabetically.
///
/// When `include_deleted` is false, returns only active subjects.
/// When true, returns all subjects (active + soft-deleted).
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn list(pool: &PgPool, include_deleted: bool) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT name FROM subjects WHERE deleted = false OR $1 ORDER BY name",
    )
    .bind(include_deleted)
    .fetch_all(pool)
    .await
}

/// Soft-delete a subject and all its schema versions. Returns the deleted version
/// numbers sorted ascending. Runs in a transaction for consistency.
///
/// # Errors
///
/// Returns a database error on connection or transaction failure.
pub async fn soft_delete(pool: &PgPool, name: &str) -> Result<Vec<i32>, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let mut versions = sqlx::query_scalar::<_, i32>(
        r"UPDATE schemas SET deleted = true
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
pub async fn hard_delete(pool: &PgPool, name: &str) -> Result<Vec<i32>, sqlx::Error> {
    let mut tx = pool.begin().await?;

    // Clean up schema_references for schemas being deleted (FK constraint).
    sqlx::query(
        r"DELETE FROM schema_references
           WHERE schema_id IN (
             SELECT id FROM schemas
             WHERE subject_id = (SELECT id FROM subjects WHERE name = $1) AND deleted = true
           )",
    )
    .bind(name)
    .execute(&mut *tx)
    .await?;

    let mut versions = sqlx::query_scalar::<_, i32>(
        r"DELETE FROM schemas
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

/// Check if a subject exists by name.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn exists(pool: &PgPool, name: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM subjects WHERE name = $1 AND deleted = false)",
    )
    .bind(name)
    .fetch_one(pool)
    .await
}
