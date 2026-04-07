//! Schema storage operations.

use sqlx::PgPool;

/// Data needed to insert a new schema version.
pub struct NewSchema<'a> {
    /// Subject this schema belongs to.
    pub subject_id: i64,
    /// Format identifier (e.g. "AVRO").
    pub schema_type: &'a str,
    /// Original schema text as submitted by the client.
    pub schema_text: &'a str,
    /// Canonical form used for deduplication.
    pub canonical_form: &'a str,
    /// Fingerprint of the canonical form.
    pub fingerprint: &'a str,
}

/// Find an existing schema ID by subject and fingerprint (for idempotency).
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn find_by_fingerprint(
    pool: &PgPool,
    subject_id: i64,
    fingerprint: &str,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT id FROM schemas WHERE subject_id = $1 AND fingerprint = $2 AND deleted = false",
    )
    .bind(subject_id)
    .bind(fingerprint)
    .fetch_optional(pool)
    .await
}

/// Insert a new schema with an atomically computed version and return its ID.
///
/// # Errors
///
/// Returns a database error on connection or constraint failure.
pub async fn insert(pool: &PgPool, schema: &NewSchema<'_>) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"INSERT INTO schemas (subject_id, version, schema_type, schema_text, canonical_form, fingerprint)
           VALUES ($1, COALESCE((SELECT MAX(version) FROM schemas WHERE subject_id = $1), 0) + 1, $2, $3, $4, $5)
           RETURNING id"#,
    )
    .bind(schema.subject_id)
    .bind(schema.schema_type)
    .bind(schema.schema_text)
    .bind(schema.canonical_form)
    .bind(schema.fingerprint)
    .fetch_one(pool)
    .await
}

/// Find a schema by its global ID (ignores soft-delete — IDs are permanent).
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>("SELECT schema_text FROM schemas WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}
