//! Registry mode storage operations.

use sqlx::PgPool;

// -- Queries --

/// Get the global registry mode.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn get_global_mode(pool: &PgPool) -> Result<String, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT COALESCE(mode, 'READWRITE') FROM config WHERE subject IS NULL",
    )
    .fetch_one(pool)
    .await
}

/// Update the global registry mode and return the new value.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn set_global_mode(pool: &PgPool, mode: &str) -> Result<String, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "UPDATE config SET mode = $1, updated_at = now() WHERE subject IS NULL RETURNING mode",
    )
    .bind(mode)
    .fetch_one(pool)
    .await
}

/// Reset the global registry mode to READWRITE (default).
///
/// Returns the **previous** mode before the reset.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn delete_global_mode(pool: &PgPool) -> Result<String, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let prev_mode: String = sqlx::query_scalar(
        "SELECT COALESCE(mode, 'READWRITE') FROM config WHERE subject IS NULL FOR UPDATE",
    )
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("UPDATE config SET mode = 'READWRITE', updated_at = now() WHERE subject IS NULL")
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(prev_mode)
}

/// Get the per-subject registry mode only (no fallback).
///
/// Returns `None` if the subject has no explicit mode override (row missing or mode is NULL).
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn get_subject_mode(pool: &PgPool, subject: &str) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT mode FROM config WHERE subject = $1 AND mode IS NOT NULL",
    )
    .bind(subject)
    .fetch_optional(pool)
    .await
}

/// Set the per-subject registry mode (upsert). Returns the new value.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn set_subject_mode(
    pool: &PgPool,
    subject: &str,
    mode: &str,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        r"INSERT INTO config (subject, mode)
          VALUES ($1, $2)
          ON CONFLICT (subject) DO UPDATE SET mode = $2, updated_at = now()
          RETURNING mode",
    )
    .bind(subject)
    .bind(mode)
    .fetch_one(pool)
    .await
}

/// Delete per-subject mode by setting it to NULL.
///
/// Returns the **previous** mode, or `None` if no per-subject mode was set.
/// Cleans up the config row if no other config remains.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn delete_subject_mode(
    pool: &PgPool,
    subject: &str,
) -> Result<Option<String>, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let prev = sqlx::query_scalar::<_, String>(
        "SELECT mode FROM config WHERE subject = $1 AND mode IS NOT NULL FOR UPDATE",
    )
    .bind(subject)
    .fetch_optional(&mut *tx)
    .await?;

    if prev.is_some() {
        sqlx::query("UPDATE config SET mode = NULL, updated_at = now() WHERE subject = $1")
            .bind(subject)
            .execute(&mut *tx)
            .await?;

        // Clean up orphan row (all nullable fields are NULL).
        sqlx::query(
            "DELETE FROM config WHERE subject = $1 AND compatibility_level IS NULL AND mode IS NULL",
        )
        .bind(subject)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(prev)
}

/// Delete per-subject mode for a subject and all child subjects (prefix match), atomically.
///
/// Returns the parent's **previous** mode, or `None` if no per-subject mode was set.
/// Uses the `^@` (starts-with) operator instead of LIKE to avoid wildcard injection.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn delete_subject_mode_recursive(
    pool: &PgPool,
    subject: &str,
) -> Result<Option<String>, sqlx::Error> {
    let mut tx = pool.begin().await?;

    // Read parent's current mode.
    let prev = sqlx::query_scalar::<_, String>(
        "SELECT mode FROM config WHERE subject = $1 AND mode IS NOT NULL FOR UPDATE",
    )
    .bind(subject)
    .fetch_optional(&mut *tx)
    .await?;

    // Clear mode on parent.
    if prev.is_some() {
        sqlx::query("UPDATE config SET mode = NULL, updated_at = now() WHERE subject = $1")
            .bind(subject)
            .execute(&mut *tx)
            .await?;
    }

    // Clear mode on all child subjects (prefix match via ^@ operator).
    sqlx::query(
        "UPDATE config SET mode = NULL, updated_at = now() WHERE subject ^@ $1 AND subject != $1 AND mode IS NOT NULL",
    )
    .bind(subject)
    .execute(&mut *tx)
    .await?;

    // Clean up orphan rows (parent + children).
    sqlx::query(
        "DELETE FROM config WHERE (subject = $1 OR (subject ^@ $1 AND subject != $1)) AND compatibility_level IS NULL AND mode IS NULL",
    )
    .bind(subject)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(prev)
}

/// Get the effective registry mode for a subject (subject-level, then global fallback).
///
/// Uses a single query to avoid TOCTOU races.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn get_effective_mode(pool: &PgPool, subject: &str) -> Result<String, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        r"SELECT COALESCE(
            (SELECT mode FROM config WHERE subject = $1 AND mode IS NOT NULL),
            (SELECT COALESCE(mode, 'READWRITE') FROM config WHERE subject IS NULL)
          )",
    )
    .bind(subject)
    .fetch_one(pool)
    .await
}
