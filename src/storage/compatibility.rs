//! Compatibility configuration storage operations.

use sqlx::PgPool;

// -- Queries --

/// Get the compatibility level for a subject, falling back to global default.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn get_level(pool: &PgPool, subject: &str) -> Result<String, sqlx::Error> {
    let level = sqlx::query_scalar::<_, String>(
        r"SELECT compatibility_level FROM config
          WHERE subject = $1 OR subject IS NULL
          ORDER BY subject IS NULL
          LIMIT 1",
    )
    .bind(subject)
    .fetch_one(pool)
    .await?;

    Ok(level)
}

/// Get the global compatibility level.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn get_global_level(pool: &PgPool) -> Result<String, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT compatibility_level FROM config WHERE subject IS NULL",
    )
    .fetch_one(pool)
    .await
}

/// Update the global compatibility level and return the new value.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn set_global_level(pool: &PgPool, level: &str) -> Result<String, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        r"UPDATE config SET compatibility_level = $1, updated_at = now()
          WHERE subject IS NULL
          RETURNING compatibility_level",
    )
    .bind(level)
    .fetch_one(pool)
    .await
}

/// Set the per-subject compatibility level (upsert). Returns the new value.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn set_subject_level(
    pool: &PgPool,
    subject: &str,
    level: &str,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        r"INSERT INTO config (subject, compatibility_level)
          VALUES ($1, $2)
          ON CONFLICT (subject) DO UPDATE SET compatibility_level = $2, updated_at = now()
          RETURNING compatibility_level",
    )
    .bind(subject)
    .bind(level)
    .fetch_one(pool)
    .await
}

/// Delete per-subject config, returning the global fallback level.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn delete_subject_level(pool: &PgPool, subject: &str) -> Result<String, sqlx::Error> {
    sqlx::query("DELETE FROM config WHERE subject = $1")
        .bind(subject)
        .execute(pool)
        .await?;

    get_global_level(pool).await
}
