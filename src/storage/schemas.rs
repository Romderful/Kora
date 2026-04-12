//! Schema storage operations.

use sqlx::{PgPool, Row};

// -- Types --

/// Data needed to insert a new schema version.
pub struct NewSchema<'a> {
    /// Format identifier (e.g. "AVRO").
    pub schema_type: &'a str,
    /// Original schema text as submitted by the client.
    pub schema_text: &'a str,
    /// Canonical form used for deduplication.
    pub canonical_form: &'a str,
    /// Fingerprint of the canonical form (for normalized dedup).
    pub fingerprint: &'a str,
    /// Fingerprint of the raw schema text (for non-normalized dedup).
    pub raw_fingerprint: &'a str,
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
    /// Schema format — omitted for AVRO (Confluent default behavior).
    #[serde(rename = "schemaType", skip_serializing_if = "is_avro")]
    pub schema_type: String,
    /// Schema references (Protobuf imports, JSON Schema `$ref`, etc.).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<crate::types::SchemaReference>,
}

fn is_avro(s: &str) -> bool {
    s == "AVRO"
}

// -- Queries --

/// Register a schema atomically: upsert subject, check for duplicate fingerprint,
/// insert schema, and store references — all in a single transaction.
///
/// Locks the subject row with `FOR UPDATE` to serialize concurrent registrations
/// and prevent version number races (even on the first schema for a subject).
///
/// Returns `(id, is_new)` — if `is_new` is false, the schema already existed (idempotent).
///
/// # Errors
///
/// Returns a database error on connection or constraint failure.
pub async fn register_schema_atomically(
    pool: &PgPool,
    subject_name: &str,
    schema: &NewSchema<'_>,
    refs: &[crate::types::SchemaReference],
    normalize: bool,
) -> Result<(i64, bool), sqlx::Error> {
    let mut tx = pool.begin().await?;

    // Upsert subject and lock the row — re-activates soft-deleted subjects.
    let subject_id = sqlx::query_scalar::<_, i64>(
        r"INSERT INTO subjects (name) VALUES ($1)
          ON CONFLICT (name) DO UPDATE SET deleted = false, updated_at = now()
          RETURNING id",
    )
    .bind(subject_name)
    .fetch_one(&mut *tx)
    .await?;

    // Lock the subject row to serialize concurrent registrations.
    // Unlike locking schema rows (empty on first insert), the subject row always exists.
    sqlx::query("SELECT 1 FROM subjects WHERE id = $1 FOR UPDATE")
        .bind(subject_id)
        .fetch_one(&mut *tx)
        .await?;

    // Idempotency: return existing ID if same fingerprint already registered.
    // When normalize=true, match on canonical fingerprint; otherwise on raw text fingerprint.
    let existing = if normalize {
        sqlx::query_scalar::<_, i64>(
            "SELECT id FROM schemas WHERE subject_id = $1 AND fingerprint = $2 AND deleted = false",
        )
        .bind(subject_id)
        .bind(schema.fingerprint)
        .fetch_optional(&mut *tx)
        .await?
    } else {
        sqlx::query_scalar::<_, i64>(
            "SELECT id FROM schemas WHERE subject_id = $1 AND raw_fingerprint = $2 AND deleted = false",
        )
        .bind(subject_id)
        .bind(schema.raw_fingerprint)
        .fetch_optional(&mut *tx)
        .await?
    };

    if let Some(id) = existing {
        tx.commit().await?;
        return Ok((id, false));
    }

    let id = sqlx::query_scalar::<_, i64>(
        r"INSERT INTO schemas (subject_id, version, schema_type, schema_text, canonical_form, fingerprint, raw_fingerprint)
           VALUES ($1, COALESCE((SELECT MAX(version) FROM schemas WHERE subject_id = $1), 0) + 1, $2, $3, $4, $5, $6)
           RETURNING id",
    )
    .bind(subject_id)
    .bind(schema.schema_type)
    .bind(schema.schema_text)
    .bind(schema.canonical_form)
    .bind(schema.fingerprint)
    .bind(schema.raw_fingerprint)
    .fetch_one(&mut *tx)
    .await?;

    // Store references inside the same transaction.
    for r in refs {
        sqlx::query(
            "INSERT INTO schema_references (schema_id, name, subject, version) VALUES ($1, $2, $3, $4)",
        )
        .bind(id)
        .bind(&r.name)
        .bind(&r.subject)
        .bind(r.version)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok((id, true))
}

/// Find a schema by subject name and version number.
///
/// When `include_deleted` is true, soft-deleted versions are included in the lookup.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn find_schema_by_subject_version(
    pool: &PgPool,
    subject: &str,
    version: i32,
    include_deleted: bool,
) -> Result<Option<SchemaVersion>, sqlx::Error> {
    let query = if include_deleted {
        r"SELECT s.id, sub.name as subject, s.version, s.schema_type, s.schema_text
           FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
           WHERE sub.name = $1 AND s.version = $2"
    } else {
        r"SELECT s.id, sub.name as subject, s.version, s.schema_type, s.schema_text
           FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
           WHERE sub.name = $1 AND s.version = $2 AND s.deleted = false"
    };
    sqlx::query(query)
        .bind(subject)
        .bind(version)
        .fetch_optional(pool)
        .await
        .map(|opt| opt.as_ref().map(row_to_schema_version))
}

/// Find the latest schema version for a subject.
///
/// When `include_deleted` is true, soft-deleted versions are considered when
/// finding the latest version.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn find_latest_schema_by_subject(
    pool: &PgPool,
    subject: &str,
    include_deleted: bool,
) -> Result<Option<SchemaVersion>, sqlx::Error> {
    let query = if include_deleted {
        r"SELECT s.id, sub.name as subject, s.version, s.schema_type, s.schema_text
           FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
           WHERE sub.name = $1
           ORDER BY s.version DESC LIMIT 1"
    } else {
        r"SELECT s.id, sub.name as subject, s.version, s.schema_type, s.schema_text
           FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
           WHERE sub.name = $1 AND s.deleted = false
           ORDER BY s.version DESC LIMIT 1"
    };
    sqlx::query(query)
        .bind(subject)
        .fetch_optional(pool)
        .await
        .map(|opt| opt.as_ref().map(row_to_schema_version))
}

/// Find a schema by subject ID and fingerprint (for check-if-registered).
///
/// When `normalize` is true, matches on canonical fingerprint; otherwise on raw fingerprint.
/// When `include_deleted` is true, soft-deleted schemas are included in the lookup.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn find_schema_by_subject_id_and_fingerprint(
    pool: &PgPool,
    subject_id: i64,
    fingerprint: &str,
    normalize: bool,
    include_deleted: bool,
) -> Result<Option<SchemaVersion>, sqlx::Error> {
    let query = match (normalize, include_deleted) {
        (true, true) =>
            r"SELECT s.id, sub.name as subject, s.version, s.schema_type, s.schema_text
               FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
               WHERE s.subject_id = $1 AND s.fingerprint = $2",
        (true, false) =>
            r"SELECT s.id, sub.name as subject, s.version, s.schema_type, s.schema_text
               FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
               WHERE s.subject_id = $1 AND s.fingerprint = $2 AND s.deleted = false",
        (false, true) =>
            r"SELECT s.id, sub.name as subject, s.version, s.schema_type, s.schema_text
               FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
               WHERE s.subject_id = $1 AND s.raw_fingerprint = $2",
        (false, false) =>
            r"SELECT s.id, sub.name as subject, s.version, s.schema_type, s.schema_text
               FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
               WHERE s.subject_id = $1 AND s.raw_fingerprint = $2 AND s.deleted = false",
    };
    sqlx::query(query)
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

/// List version numbers for a subject, sorted ascending, with pagination.
///
/// - `include_deleted`: when true, include soft-deleted versions in results.
/// - `deleted_only`: when true, return ONLY soft-deleted versions (takes precedence).
/// - `deleted_as_negative`: when true (requires `include_deleted`), soft-deleted
///   versions appear as negative numbers (e.g., version 2 deleted → -2).
/// - `offset` defaults to 0, `limit` of -1 means unlimited.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn list_schema_versions(
    pool: &PgPool,
    subject: &str,
    include_deleted: bool,
    deleted_only: bool,
    deleted_as_negative: bool,
    offset: i64,
    limit: i64,
) -> Result<Vec<i32>, sqlx::Error> {
    if deleted_only && deleted_as_negative {
        // All results are deleted → return as negative numbers.
        if limit >= 0 {
            sqlx::query_scalar(
                r"SELECT -s.version as version FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
                   WHERE sub.name = $1 AND s.deleted = true
                   ORDER BY s.version OFFSET $2 LIMIT $3",
            )
            .bind(subject)
            .bind(offset)
            .bind(limit)
            .fetch_all(pool)
            .await
        } else {
            sqlx::query_scalar(
                r"SELECT -s.version as version FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
                   WHERE sub.name = $1 AND s.deleted = true
                   ORDER BY s.version OFFSET $2",
            )
            .bind(subject)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
    } else if deleted_only {
        if limit >= 0 {
            sqlx::query_scalar(
                r"SELECT s.version FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
                   WHERE sub.name = $1 AND s.deleted = true
                   ORDER BY s.version OFFSET $2 LIMIT $3",
            )
            .bind(subject)
            .bind(offset)
            .bind(limit)
            .fetch_all(pool)
            .await
        } else {
            sqlx::query_scalar(
                r"SELECT s.version FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
                   WHERE sub.name = $1 AND s.deleted = true
                   ORDER BY s.version OFFSET $2",
            )
            .bind(subject)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
    } else if deleted_as_negative && include_deleted {
        // Soft-deleted versions appear as negative numbers, ordered by absolute value.
        if limit >= 0 {
            sqlx::query_scalar(
                r"SELECT CASE WHEN s.deleted THEN -s.version ELSE s.version END as version
                   FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
                   WHERE sub.name = $1
                   ORDER BY abs(s.version) OFFSET $2 LIMIT $3",
            )
            .bind(subject)
            .bind(offset)
            .bind(limit)
            .fetch_all(pool)
            .await
        } else {
            sqlx::query_scalar(
                r"SELECT CASE WHEN s.deleted THEN -s.version ELSE s.version END as version
                   FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
                   WHERE sub.name = $1
                   ORDER BY abs(s.version) OFFSET $2",
            )
            .bind(subject)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
    } else if limit >= 0 {
        sqlx::query_scalar(
            r"SELECT s.version FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
               WHERE sub.name = $1 AND (s.deleted = false OR $2)
               ORDER BY s.version OFFSET $3 LIMIT $4",
        )
        .bind(subject)
        .bind(include_deleted)
        .bind(offset)
        .bind(limit)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_scalar(
            r"SELECT s.version FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
               WHERE sub.name = $1 AND (s.deleted = false OR $2)
               ORDER BY s.version OFFSET $3",
        )
        .bind(subject)
        .bind(include_deleted)
        .bind(offset)
        .fetch_all(pool)
        .await
    }
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

/// Check if a specific version is soft-deleted under a subject.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn version_is_soft_deleted(pool: &PgPool, subject: &str, version: i32) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        r"SELECT EXISTS(
            SELECT 1 FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
            WHERE sub.name = $1 AND s.version = $2 AND s.deleted = true
        )",
    )
    .bind(subject)
    .bind(version)
    .fetch_one(pool)
    .await
}

/// Check if a specific version is active (not soft-deleted) under a subject.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn version_is_active(pool: &PgPool, subject: &str, version: i32) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        r"SELECT EXISTS(
            SELECT 1 FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
            WHERE sub.name = $1 AND s.version = $2 AND s.deleted = false
        )",
    )
    .bind(subject)
    .bind(version)
    .fetch_one(pool)
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

/// Get the maximum schema ID in the registry.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn find_max_schema_id(pool: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, Option<i64>>("SELECT MAX(id) FROM schemas")
        .fetch_one(pool)
        .await
        .map(|opt| opt.unwrap_or(0))
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

/// Find all subjects that use a given schema ID, with pagination.
///
/// - `include_deleted`: when true, include soft-deleted subjects/versions.
/// - `subject_filter`: when `Some`, filter to a specific subject name.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn find_subjects_by_schema_id(
    pool: &PgPool,
    id: i64,
    include_deleted: bool,
    subject_filter: Option<&str>,
    offset: i64,
    limit: i64,
) -> Result<Vec<String>, sqlx::Error> {
    match (subject_filter, limit >= 0) {
        (Some(filter), true) => sqlx::query_scalar(
            r"SELECT sub.name FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
               WHERE s.id = $1 AND (s.deleted = false OR $2) AND (sub.deleted = false OR $2)
                 AND sub.name = $3
               ORDER BY sub.name OFFSET $4 LIMIT $5",
        )
        .bind(id)
        .bind(include_deleted)
        .bind(filter)
        .bind(offset)
        .bind(limit)
        .fetch_all(pool)
        .await,

        (Some(filter), false) => sqlx::query_scalar(
            r"SELECT sub.name FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
               WHERE s.id = $1 AND (s.deleted = false OR $2) AND (sub.deleted = false OR $2)
                 AND sub.name = $3
               ORDER BY sub.name OFFSET $4",
        )
        .bind(id)
        .bind(include_deleted)
        .bind(filter)
        .bind(offset)
        .fetch_all(pool)
        .await,

        (None, true) => sqlx::query_scalar(
            r"SELECT sub.name FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
               WHERE s.id = $1 AND (s.deleted = false OR $2) AND (sub.deleted = false OR $2)
               ORDER BY sub.name OFFSET $3 LIMIT $4",
        )
        .bind(id)
        .bind(include_deleted)
        .bind(offset)
        .bind(limit)
        .fetch_all(pool)
        .await,

        (None, false) => sqlx::query_scalar(
            r"SELECT sub.name FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
               WHERE s.id = $1 AND (s.deleted = false OR $2) AND (sub.deleted = false OR $2)
               ORDER BY sub.name OFFSET $3",
        )
        .bind(id)
        .bind(include_deleted)
        .bind(offset)
        .fetch_all(pool)
        .await,
    }
}

/// Find all subject-version pairs that use a given schema ID, with pagination.
///
/// - `include_deleted`: when true, include soft-deleted subjects/versions.
/// - `subject_filter`: when `Some`, filter to a specific subject name.
///
/// # Errors
///
/// Returns a database error on connection failure.
pub async fn find_versions_by_schema_id(
    pool: &PgPool,
    id: i64,
    include_deleted: bool,
    subject_filter: Option<&str>,
    offset: i64,
    limit: i64,
) -> Result<Vec<SubjectVersion>, sqlx::Error> {
    let rows = match (subject_filter, limit >= 0) {
        (Some(filter), true) => sqlx::query(
            r"SELECT sub.name as subject, s.version
               FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
               WHERE s.id = $1 AND (s.deleted = false OR $2) AND (sub.deleted = false OR $2)
                 AND sub.name = $3
               ORDER BY sub.name, s.version OFFSET $4 LIMIT $5",
        )
        .bind(id)
        .bind(include_deleted)
        .bind(filter)
        .bind(offset)
        .bind(limit)
        .fetch_all(pool)
        .await?,

        (Some(filter), false) => sqlx::query(
            r"SELECT sub.name as subject, s.version
               FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
               WHERE s.id = $1 AND (s.deleted = false OR $2) AND (sub.deleted = false OR $2)
                 AND sub.name = $3
               ORDER BY sub.name, s.version OFFSET $4",
        )
        .bind(id)
        .bind(include_deleted)
        .bind(filter)
        .bind(offset)
        .fetch_all(pool)
        .await?,

        (None, true) => sqlx::query(
            r"SELECT sub.name as subject, s.version
               FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
               WHERE s.id = $1 AND (s.deleted = false OR $2) AND (sub.deleted = false OR $2)
               ORDER BY sub.name, s.version OFFSET $3 LIMIT $4",
        )
        .bind(id)
        .bind(include_deleted)
        .bind(offset)
        .bind(limit)
        .fetch_all(pool)
        .await?,

        (None, false) => sqlx::query(
            r"SELECT sub.name as subject, s.version
               FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
               WHERE s.id = $1 AND (s.deleted = false OR $2) AND (sub.deleted = false OR $2)
               ORDER BY sub.name, s.version OFFSET $3",
        )
        .bind(id)
        .bind(include_deleted)
        .bind(offset)
        .fetch_all(pool)
        .await?,
    };

    Ok(rows.iter().map(|row| SubjectVersion {
        subject: row.get("subject"),
        version: row.get("version"),
    }).collect())
}

// -- Helpers --

fn row_to_schema_version(row: &sqlx::postgres::PgRow) -> SchemaVersion {
    SchemaVersion {
        subject: row.get("subject"),
        id: row.get("id"),
        version: row.get("version"),
        schema: row.get("schema_text"),
        schema_type: row.get("schema_type"),
        references: Vec::new(),
    }
}
