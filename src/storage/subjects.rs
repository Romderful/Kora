//! Subject storage operations.

use sqlx::PgPool;

/// Insert a subject if it doesn't exist and return its ID.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn upsert(pool: &PgPool, name: &str) -> Result<i64, sqlx::Error> {
    let id = sqlx::query_scalar::<_, i64>(
        r#"WITH ins AS (
             INSERT INTO subjects (name) VALUES ($1)
             ON CONFLICT (name) DO NOTHING
             RETURNING id
           )
           SELECT id FROM ins
           UNION ALL
           SELECT id FROM subjects WHERE name = $1
           LIMIT 1"#,
    )
    .bind(name)
    .fetch_one(pool)
    .await?;

    Ok(id)
}
