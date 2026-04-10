//! Schema storage operations.

use sqlx::{PgPool, Row};

// -- Types --

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

/// A subject-version pair, returned by schema ID cross-reference lookups.
#[derive(Debug, serde::Serialize)]
pub struct SubjectVersion {
    /// Subject name.
    pub subject: String,
    /// Version number within the subject.
    pub version: i32,
}

/// A schema with its subject context, returned by version lookups.
#[derive(Debug, serde::Serialize)]
pub struct SchemaVersion {
    /// Subject name.
    pub subject: String,
    /// Global schema ID.
    pub id: i64,
    /// Version number within the subject.
    pub version: i32,
    /// Raw schema text.
    pub schema: String,
    /// Schema format (e.g. "AVRO").
    #[serde(rename = "schemaType")]
    pub schema_type: String,
}

// -- Queries --

/// Find an existing schema ID by subject and fingerprint (for idempotency).
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn find_schema_id_by_subject_id_and_fingerprint(
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
pub async fn insert_schema(pool: &PgPool, schema: &NewSchema<'_>) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r"INSERT INTO schemas (subject_id, version, schema_type, schema_text, canonical_form, fingerprint)
           VALUES ($1, COALESCE((SELECT MAX(version) FROM schemas WHERE subject_id = $1), 0) + 1, $2, $3, $4, $5)
           RETURNING id",
    )
    .bind(schema.subject_id)
    .bind(schema.schema_type)
    .bind(schema.schema_text)
    .bind(schema.canonical_form)
    .bind(schema.fingerprint)
    .fetch_one(pool)
    .await
}

/// Find a schema by subject name and version number.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn find_schema_by_subject_version(
    pool: &PgPool,
    subject: &str,
    version: i32,
) -> Result<Option<SchemaVersion>, sqlx::Error> {
    sqlx::query(
        r"SELECT s.id, sub.name as subject, s.version, s.schema_type, s.schema_text
           FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
           WHERE sub.name = $1 AND s.version = $2 AND s.deleted = false",
    )
    .bind(subject)
    .bind(version)
    .fetch_optional(pool)
    .await
    .map(|opt| opt.as_ref().map(row_to_schema_version))
}

/// Find the latest schema version for a subject.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn find_latest_schema_by_subject(
    pool: &PgPool,
    subject: &str,
) -> Result<Option<SchemaVersion>, sqlx::Error> {
    sqlx::query(
        r"SELECT s.id, sub.name as subject, s.version, s.schema_type, s.schema_text
           FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
           WHERE sub.name = $1 AND s.deleted = false
           ORDER BY s.version DESC LIMIT 1",
    )
    .bind(subject)
    .fetch_optional(pool)
    .await
    .map(|opt| opt.as_ref().map(row_to_schema_version))
}

/// Find a schema by subject ID and fingerprint (for check-if-registered).
///
/// Returns the full schema version details, unlike
/// `find_schema_id_by_subject_id_and_fingerprint` which returns only the ID.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn find_schema_by_subject_id_and_fingerprint(
    pool: &PgPool,
    subject_id: i64,
    fingerprint: &str,
) -> Result<Option<SchemaVersion>, sqlx::Error> {
    sqlx::query(
        r"SELECT s.id, sub.name as subject, s.version, s.schema_type, s.schema_text
           FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
           WHERE s.subject_id = $1 AND s.fingerprint = $2 AND s.deleted = false",
    )
    .bind(subject_id)
    .bind(fingerprint)
    .fetch_optional(pool)
    .await
    .map(|opt| opt.as_ref().map(row_to_schema_version))
}

/// Soft-delete the latest schema version for a subject. Returns the version number if found.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn soft_delete_latest_schema(pool: &PgPool, subject: &str) -> Result<Option<i32>, sqlx::Error> {
    sqlx::query_scalar::<_, i32>(
        r"UPDATE schemas SET deleted = true
           WHERE id = (
             SELECT s.id FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
             WHERE sub.name = $1 AND s.deleted = false
             ORDER BY s.version DESC LIMIT 1
           )
           RETURNING version",
    )
    .bind(subject)
    .fetch_optional(pool)
    .await
}

/// Hard-delete a soft-deleted schema version. Returns the version number if found.
///
/// Only operates on rows where `deleted = true`.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn hard_delete_schema_version(
    pool: &PgPool,
    subject: &str,
    version: i32,
) -> Result<Option<i32>, sqlx::Error> {
    let mut tx = pool.begin().await?;

    // Clean up schema_references for the schema being deleted (FK constraint).
    sqlx::query(
        r"DELETE FROM schema_references
           WHERE schema_id = (
             SELECT id FROM schemas
             WHERE subject_id = (SELECT id FROM subjects WHERE name = $1)
               AND version = $2 AND deleted = true
           )",
    )
    .bind(subject)
    .bind(version)
    .execute(&mut *tx)
    .await?;

    let result = sqlx::query_scalar::<_, i32>(
        r"DELETE FROM schemas
           WHERE subject_id = (SELECT id FROM subjects WHERE name = $1)
             AND version = $2 AND deleted = true
           RETURNING version",
    )
    .bind(subject)
    .bind(version)
    .fetch_optional(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(result)
}

/// List version numbers for a subject, sorted ascending.
///
/// When `include_deleted` is false, returns only active versions.
/// When true, returns all versions (active + soft-deleted).
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn list_schema_versions(
    pool: &PgPool,
    subject: &str,
    include_deleted: bool,
) -> Result<Vec<i32>, sqlx::Error> {
    sqlx::query_scalar::<_, i32>(
        r"SELECT s.version FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
           WHERE sub.name = $1 AND (s.deleted = false OR $2) ORDER BY s.version",
    )
    .bind(subject)
    .bind(include_deleted)
    .fetch_all(pool)
    .await
}

/// Soft-delete a single schema version. Returns the version number if found.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn soft_delete_schema_version(
    pool: &PgPool,
    subject: &str,
    version: i32,
) -> Result<Option<i32>, sqlx::Error> {
    sqlx::query_scalar::<_, i32>(
        r"UPDATE schemas SET deleted = true
           WHERE subject_id = (SELECT id FROM subjects WHERE name = $1)
             AND version = $2 AND deleted = false
           RETURNING version",
    )
    .bind(subject)
    .bind(version)
    .fetch_optional(pool)
    .await
}

/// Find a schema by its global ID (ignores soft-delete — IDs are permanent).
/// Returns `(schema_text, schema_type)`.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn find_schema_by_id(pool: &PgPool, id: i64) -> Result<Option<(String, String)>, sqlx::Error> {
    sqlx::query("SELECT schema_text, schema_type FROM schemas WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map(|opt| opt.as_ref().map(|row| (row.get("schema_text"), row.get("schema_type"))))
}

/// Check if a schema exists by global ID (ignores soft-delete — IDs are permanent).
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn schema_exists(pool: &PgPool, id: i64) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM schemas WHERE id = $1)")
        .bind(id)
        .fetch_one(pool)
        .await
}

/// Find all subjects that use a given schema ID (excludes soft-deleted).
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn find_subjects_by_schema_id(pool: &PgPool, id: i64) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        r"SELECT sub.name
           FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
           WHERE s.id = $1 AND s.deleted = false AND sub.deleted = false
           ORDER BY sub.name",
    )
    .bind(id)
    .fetch_all(pool)
    .await
}

/// Find all subject-version pairs that use a given schema ID (excludes soft-deleted).
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn find_versions_by_schema_id(
    pool: &PgPool,
    id: i64,
) -> Result<Vec<SubjectVersion>, sqlx::Error> {
    sqlx::query(
        r"SELECT sub.name as subject, s.version
           FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
           WHERE s.id = $1 AND s.deleted = false AND sub.deleted = false
           ORDER BY sub.name, s.version",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map(|rows| {
        rows.iter()
            .map(|row| SubjectVersion {
                subject: row.get("subject"),
                version: row.get("version"),
            })
            .collect()
    })
}

// -- Helpers --

fn row_to_schema_version(row: &sqlx::postgres::PgRow) -> SchemaVersion {
    SchemaVersion {
        subject: row.get("subject"),
        id: row.get("id"),
        version: row.get("version"),
        schema: row.get("schema_text"),
        schema_type: row.get("schema_type"),
    }
}
