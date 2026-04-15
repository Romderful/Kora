//! Subject-related API handlers.

use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection},
    response::IntoResponse,
};
use serde::Deserialize;
use sqlx::PgPool;

use crate::api::mode::enforce_writable;
use crate::error::KoraError;
use crate::schema::{self, SchemaFormat};
use crate::storage::{compatibility, references, schemas, subjects};
use crate::types::SchemaReference;

// -- Types --

/// Request body for schema registration and check endpoints.
#[derive(Debug, Deserialize)]
pub struct SchemaRequest {
    /// The raw schema string (JSON-encoded).
    pub schema: String,
    /// Schema format — defaults to AVRO when absent.
    #[serde(rename = "schemaType")]
    pub schema_type: Option<String>,
    /// Optional schema references (for Protobuf imports, JSON Schema `$ref`, etc.).
    #[serde(default)]
    pub references: Option<Vec<SchemaReference>>,
}

/// Query parameters for `GET /subjects` with Confluent-specific filters.
#[derive(Debug, Deserialize)]
pub struct ListSubjectsParams {
    /// When true, include soft-deleted subjects in the response.
    #[serde(default)]
    pub deleted: bool,
    /// When true, return ONLY soft-deleted subjects (takes precedence over `deleted`).
    #[serde(default, rename = "deletedOnly")]
    pub deleted_only: bool,
    /// Accept-and-ignore: Confluent OSS param that overlaps with `deleted`/`deletedOnly`.
    #[serde(default, rename = "lookupDeletedSubject")]
    _lookup_deleted_subject: bool,
    /// Subject name prefix filter. Default `:*:` means match all.
    #[serde(default = "default_subject_prefix", rename = "subjectPrefix")]
    pub subject_prefix: String,
    /// Pagination offset (default 0).
    #[serde(default)]
    pub offset: i64,
    /// Pagination limit (-1 = unlimited, default).
    #[serde(default = "default_limit")]
    pub limit: i64,
}

pub(crate) fn default_subject_prefix() -> String {
    ":*:".to_string()
}

/// Query parameters for `GET /subjects/{subject}/versions` with Confluent-specific filters.
#[derive(Debug, Deserialize)]
pub struct ListVersionsParams {
    /// When true, include soft-deleted versions in the response.
    #[serde(default)]
    pub deleted: bool,
    /// When true, return ONLY soft-deleted versions (takes precedence over `deleted`).
    #[serde(default, rename = "deletedOnly")]
    pub deleted_only: bool,
    /// When true, soft-deleted versions appear as negative numbers.
    #[serde(default, rename = "deletedAsNegative")]
    pub deleted_as_negative: bool,
    /// Pagination offset (default 0).
    #[serde(default)]
    pub offset: i64,
    /// Pagination limit (-1 = unlimited, default).
    #[serde(default = "default_limit")]
    pub limit: i64,
}

/// Default limit: -1 (unlimited). Shared by all pagination param structs.
pub(crate) const fn default_limit() -> i64 {
    -1
}

/// Query parameters for schema registration (`POST /subjects/{subject}/versions`).
#[derive(Debug, Deserialize)]
pub struct RegisterParams {
    /// When true, normalize schema before fingerprint comparison (already default behavior).
    #[serde(default)]
    pub normalize: bool,
    /// Schema format hint (accept-and-ignore — format determined from body's `schemaType`).
    #[serde(default)]
    pub format: Option<String>,
}

/// Query parameters for schema check (`POST /subjects/{subject}`).
#[derive(Debug, Deserialize)]
pub struct CheckParams {
    /// When true, normalize schema before fingerprint comparison (already default behavior).
    #[serde(default)]
    pub normalize: bool,
    /// When true, include soft-deleted schemas in the lookup.
    #[serde(default)]
    pub deleted: bool,
    /// Schema format hint (accept-and-ignore).
    #[serde(default)]
    pub format: Option<String>,
}

/// Query parameters for `GET /subjects/{subject}/versions/{version}`.
#[derive(Debug, Deserialize)]
pub struct GetVersionParams {
    /// When true, include soft-deleted versions in the lookup.
    #[serde(default)]
    pub deleted: bool,
    /// Schema output format (accept-and-ignore — reference resolution not yet implemented).
    #[serde(default)]
    pub format: Option<String>,
    /// Reference output format (accept-and-ignore).
    #[serde(default, rename = "referenceFormat")]
    pub reference_format: Option<String>,
}

/// Query parameters for DELETE endpoints supporting `?permanent=true`.
#[derive(Debug, Deserialize)]
pub struct PermanentParams {
    /// When true, hard-delete (requires prior soft-delete).
    #[serde(default)]
    pub permanent: bool,
}

/// Query parameters for `GET /subjects/{subject}/versions/{version}/referencedby`.
#[derive(Debug, Deserialize)]
pub struct ReferencedByParams {
    /// When true, include soft-deleted referencing schema versions.
    #[serde(default)]
    pub deleted: bool,
    /// Pagination offset (default 0).
    #[serde(default)]
    pub offset: i64,
    /// Pagination limit (-1 = unlimited, default).
    #[serde(default = "default_limit")]
    pub limit: i64,
}

// -- Handlers --

/// Register a schema under a subject.
///
/// `POST /subjects/{subject}/versions`
///
/// # Errors
///
/// Returns `KoraError::InvalidSchema` (422) for unparseable schemas or
/// `KoraError::BackendDataStore` (500) for database failures.
pub async fn register_schema(
    State(pool): State<PgPool>,
    Path(subject): Path<String>,
    Query(params): Query<RegisterParams>,
    body: Result<Json<SchemaRequest>, JsonRejection>,
) -> Result<impl IntoResponse, KoraError> {
    let Json(body) = body.map_err(|e| KoraError::InvalidSchema(e.body_text()))?;

    validate_subject(&subject)?;

    // Enforce registry mode before expensive parsing/compat checks.
    enforce_writable(&pool, &subject).await?;

    let format = SchemaFormat::from_optional(body.schema_type.as_deref())?;
    let parsed = schema::parse(format, &body.schema)?;

    // Resolve effective normalize: explicit param OR subject/global config.
    let normalize =
        params.normalize || compatibility::get_effective_normalize(&pool, &subject).await?;

    // Enforce compatibility mode before registration.
    let level = compatibility::get_effective_compatibility(&pool, &subject).await?;
    let direction = schema::CompatDirection::from_level(&level);
    if direction != schema::CompatDirection::None {
        let is_transitive = level.contains("TRANSITIVE");
        let versions_to_check: Vec<schemas::SchemaVersion> = if is_transitive {
            // Transitive: check against ALL active versions.
            let nums =
                schemas::list_schema_versions(&pool, &subject, false, false, false, 0, -1).await?;
            let mut versions = Vec::with_capacity(nums.len());
            for v in nums {
                if let Some(sv) =
                    schemas::find_schema_by_subject_version(&pool, &subject, v, false).await?
                {
                    versions.push(sv);
                }
            }
            versions
        } else {
            // Non-transitive: check against latest version only.
            schemas::find_latest_schema_by_subject(&pool, &subject, false)
                .await?
                .into_iter()
                .collect()
        };

        for existing in &versions_to_check {
            // Skip type-mismatched versions (subject may have mixed types under NONE then switched).
            let Ok(existing_format) = SchemaFormat::from_optional(Some(&existing.schema_type))
            else {
                continue;
            };
            if existing_format != format {
                continue;
            }
            let result =
                schema::check_compatibility(format, &body.schema, &existing.schema, direction)?;
            if !result.is_compatible {
                return Err(KoraError::IncompatibleSchema);
            }
        }
    }

    // Validate references before any writes.
    let refs = body.references.as_deref().unwrap_or_default();
    if !refs.is_empty() {
        references::validate_references(&pool, refs).await?;
    }

    let (id, version, _is_new) = schemas::register_schema_atomically(
        &pool,
        &subject,
        &schemas::NewSchema {
            schema_type: format.as_str(),
            schema_text: &body.schema,
            canonical_form: &parsed.canonical_form,
            fingerprint: &parsed.fingerprint,
            raw_fingerprint: &parsed.raw_fingerprint,
        },
        refs,
        normalize,
    )
    .await?;

    // Confluent RegisterSchemaResponse: id, version, schemaType, schema always present.
    // References included when non-empty (NON_EMPTY).
    let mut resp = serde_json::json!({
        "id": id,
        "version": version,
        "schemaType": format.as_str(),
        "schema": body.schema,
    });
    if !refs.is_empty() {
        resp["references"] = serde_json::json!(refs);
    }
    Ok(Json(resp))
}

/// Check if a schema is registered under a subject.
///
/// `POST /subjects/{subject}`
///
/// # Errors
///
/// Returns `KoraError::SubjectNotFound` (40401) if the subject doesn't exist,
/// or `KoraError::SchemaNotFound` (40403) if the schema is not registered.
pub async fn check_schema(
    State(pool): State<PgPool>,
    Path(subject): Path<String>,
    Query(params): Query<CheckParams>,
    body: Result<Json<SchemaRequest>, JsonRejection>,
) -> Result<impl IntoResponse, KoraError> {
    let Json(body) = body.map_err(|e| KoraError::InvalidSchema(e.body_text()))?;

    validate_subject(&subject)?;

    let format = SchemaFormat::from_optional(body.schema_type.as_deref())?;
    let parsed = schema::parse(format, &body.schema)?;

    // Resolve effective normalize: explicit param OR subject/global config.
    let normalize =
        params.normalize || compatibility::get_effective_normalize(&pool, &subject).await?;

    let fp = if normalize {
        &parsed.fingerprint
    } else {
        &parsed.raw_fingerprint
    };

    let subject_id = subjects::find_subject_id_by_name(&pool, &subject, params.deleted)
        .await?
        .ok_or(KoraError::SubjectNotFound)?;

    let sv = schemas::find_schema_by_subject_id_and_fingerprint(
        &pool,
        subject_id,
        fp,
        normalize,
        params.deleted,
    )
    .await?
    .ok_or(KoraError::SchemaNotFound)?;

    Ok(Json(load_references(&pool, sv).await?))
}

/// List registered subjects.
///
/// `GET /subjects` — non-deleted subjects (default).
/// `GET /subjects?deleted=true` — all subjects (including soft-deleted).
///
/// # Errors
///
/// Returns `KoraError::BackendDataStore` (500) for database failures.
pub async fn list_subjects(
    State(pool): State<PgPool>,
    Query(params): Query<ListSubjectsParams>,
) -> Result<impl IntoResponse, KoraError> {
    let prefix = if params.subject_prefix == ":*:" || params.subject_prefix.is_empty() {
        None
    } else {
        Some(params.subject_prefix.as_str())
    };
    let names = subjects::list_subjects(
        &pool,
        params.deleted,
        params.deleted_only,
        prefix,
        params.offset.max(0),
        params.limit,
    )
    .await?;
    Ok(Json(names))
}

/// List all versions of a subject.
///
/// `GET /subjects/{subject}/versions`
///
/// # Errors
///
/// Returns `KoraError::SubjectNotFound` (40401) if the subject doesn't exist,
/// or `KoraError::BackendDataStore` (500) for database failures.
pub async fn list_versions(
    State(pool): State<PgPool>,
    Path(subject): Path<String>,
    Query(params): Query<ListVersionsParams>,
) -> Result<impl IntoResponse, KoraError> {
    validate_subject(&subject)?;

    if !subjects::subject_exists(&pool, &subject, params.deleted || params.deleted_only).await? {
        return Err(KoraError::SubjectNotFound);
    }

    let versions = schemas::list_schema_versions(
        &pool,
        &subject,
        params.deleted,
        params.deleted_only,
        params.deleted_as_negative,
        params.offset.max(0),
        params.limit,
    )
    .await?;
    Ok(Json(versions))
}

/// Retrieve a schema by subject and version.
///
/// `GET /subjects/{subject}/versions/{version}`
///
/// The version can be a number or "latest".
///
/// # Errors
///
/// Returns `KoraError::SubjectNotFound` (40401) if the subject doesn't exist,
/// or `KoraError::VersionNotFound` (40402) if the version doesn't exist.
pub async fn get_schema_by_version(
    State(pool): State<PgPool>,
    Path((subject, version)): Path<(String, String)>,
    Query(params): Query<GetVersionParams>,
) -> Result<impl IntoResponse, KoraError> {
    validate_subject(&subject)?;

    let row = if version == "latest" {
        schemas::find_latest_schema_by_subject(&pool, &subject, params.deleted).await?
    } else {
        let v = parse_version(&version)?;
        schemas::find_schema_by_subject_version(&pool, &subject, v, params.deleted).await?
    };

    if let Some(sv) = row {
        Ok(Json(load_references(&pool, sv).await?))
    } else if subjects::subject_exists(&pool, &subject, params.deleted).await? {
        Err(KoraError::VersionNotFound)
    } else {
        Err(KoraError::SubjectNotFound)
    }
}

/// Soft-delete a subject and all its versions.
///
/// `DELETE /subjects/{subject}`
///
/// # Errors
///
/// Returns `KoraError::SubjectNotFound` (40401) if the subject doesn't exist
/// (or isn't soft-deleted when `permanent=true`).
pub async fn delete_subject(
    State(pool): State<PgPool>,
    Path(subject): Path<String>,
    Query(params): Query<PermanentParams>,
) -> Result<impl IntoResponse, KoraError> {
    validate_subject(&subject)?;
    enforce_writable(&pool, &subject).await?;

    if params.permanent {
        // Confluent requires subject to be soft-deleted first (40405).
        // subject_exists returns false for both nonexistent AND soft-deleted subjects,
        // so we must distinguish: if active → 40405, if nonexistent → let hard_delete handle 40401.
        if subjects::subject_exists(&pool, &subject, false).await? {
            return Err(KoraError::SubjectNotSoftDeleted(subject));
        }
        // Check if any version of this subject is referenced by other schemas.
        let versions_to_delete =
            schemas::list_schema_versions(&pool, &subject, true, false, false, 0, -1).await?;
        for v in &versions_to_delete {
            if references::is_version_referenced(&pool, &subject, *v).await? {
                return Err(KoraError::ReferenceExists(format!("{subject} version {v}")));
            }
        }
        let versions = subjects::hard_delete_subject(&pool, &subject).await?;
        if versions.is_empty() {
            return Err(KoraError::SubjectNotFound);
        }
        Ok(Json(versions))
    } else {
        if !subjects::subject_exists(&pool, &subject, false).await? {
            return Err(subject_not_found_or_soft_deleted(&pool, &subject).await);
        }
        let versions = subjects::soft_delete_subject(&pool, &subject).await?;
        Ok(Json(versions))
    }
}

/// Delete a single schema version (soft or hard).
///
/// `DELETE /subjects/{subject}/versions/{version}`
/// `DELETE /subjects/{subject}/versions/{version}?permanent=true`
///
/// # Errors
///
/// Returns `KoraError::SubjectNotFound` (40401) or `KoraError::VersionNotFound` (40402).
pub async fn delete_version(
    State(pool): State<PgPool>,
    Path((subject, version)): Path<(String, String)>,
    Query(params): Query<PermanentParams>,
) -> Result<impl IntoResponse, KoraError> {
    validate_subject(&subject)?;
    enforce_writable(&pool, &subject).await?;

    let deleted = if params.permanent {
        let v = parse_version(&version)?;
        // Subject must exist (even soft-deleted) for hard-delete to make sense.
        if !subjects::subject_exists(&pool, &subject, true).await? {
            return Err(KoraError::SubjectNotFound);
        }
        // Confluent requires version to be soft-deleted first (40407).
        if schemas::version_is_active(&pool, &subject, v).await? {
            return Err(KoraError::SchemaVersionNotSoftDeleted(subject, v));
        }
        if references::is_version_referenced(&pool, &subject, v).await? {
            return Err(KoraError::ReferenceExists(format!("{subject} version {v}")));
        }
        schemas::hard_delete_schema_version(&pool, &subject, v).await?
    } else {
        if !subjects::subject_exists(&pool, &subject, false).await? {
            return Err(KoraError::SubjectNotFound);
        }
        if version == "latest" {
            schemas::soft_delete_latest_schema(&pool, &subject).await?
        } else {
            let v = parse_version(&version)?;
            // Return 40406 if version is already soft-deleted.
            if schemas::version_is_soft_deleted(&pool, &subject, v).await? {
                return Err(KoraError::SchemaVersionSoftDeleted(subject, v));
            }
            schemas::soft_delete_schema_version(&pool, &subject, v).await?
        }
    }
    .ok_or(KoraError::VersionNotFound)?;

    Ok(Json(deleted))
}

/// Retrieve schema text by subject and version.
///
/// `GET /subjects/{subject}/versions/{version}/schema`
///
/// Returns the schema text only — no metadata wrapper.
///
/// # Errors
///
/// Returns `KoraError::SubjectNotFound` (40401) if the subject doesn't exist,
/// or `KoraError::VersionNotFound` (40402) if the version doesn't exist.
pub async fn get_schema_text_by_version(
    State(pool): State<PgPool>,
    Path((subject, version)): Path<(String, String)>,
    Query(params): Query<GetVersionParams>,
) -> Result<impl IntoResponse, KoraError> {
    validate_subject(&subject)?;

    let row = if version == "latest" {
        schemas::find_latest_schema_by_subject(&pool, &subject, params.deleted).await?
    } else {
        let v = parse_version(&version)?;
        schemas::find_schema_by_subject_version(&pool, &subject, v, params.deleted).await?
    };

    if let Some(sv) = row {
        Ok(Json(sv.schema))
    } else if subjects::subject_exists(&pool, &subject, params.deleted).await? {
        Err(KoraError::VersionNotFound)
    } else {
        Err(KoraError::SubjectNotFound)
    }
}

/// List schema IDs that reference a given subject/version.
///
/// `GET /subjects/{subject}/versions/{version}/referencedby`
///
/// Returns an array of global schema IDs (content IDs) whose schemas contain
/// a reference to the given subject at the given version.
///
/// # Errors
///
/// Returns `KoraError::SubjectNotFound` (40401) if the subject doesn't exist,
/// `KoraError::VersionNotFound` (40402) if the version doesn't exist,
/// or `KoraError::InvalidVersion` (42202) for invalid version strings.
pub async fn get_referencing_ids_by_version(
    State(pool): State<PgPool>,
    Path((subject, version)): Path<(String, String)>,
    Query(params): Query<ReferencedByParams>,
) -> Result<impl IntoResponse, KoraError> {
    validate_subject(&subject)?;
    let v = parse_version(&version)?;

    let ids = references::find_referencing_schema_ids(
        &pool,
        &subject,
        v,
        params.deleted,
        params.offset.max(0),
        params.limit,
    )
    .await?;

    // Lazy existence check: only when result is empty (avoids extra queries on happy path).
    if ids.is_empty() {
        if !subjects::subject_exists(&pool, &subject, false).await? {
            return Err(KoraError::SubjectNotFound);
        }
        if schemas::find_schema_by_subject_version(&pool, &subject, v, false)
            .await?
            .is_none()
        {
            return Err(KoraError::VersionNotFound);
        }
    }

    Ok(Json(ids))
}

// -- Helpers --

/// Populate schema references on a `SchemaVersion` before returning to the client.
async fn load_references(
    pool: &PgPool,
    mut sv: schemas::SchemaVersion,
) -> Result<schemas::SchemaVersion, KoraError> {
    sv.references = references::find_references_by_schema_id(pool, sv.id).await?;
    Ok(sv)
}

/// Return `SubjectNotFound` or `SubjectSoftDeleted` based on subject state.
async fn subject_not_found_or_soft_deleted(pool: &PgPool, subject: &str) -> KoraError {
    if subjects::subject_is_soft_deleted(pool, subject)
        .await
        .unwrap_or(false)
    {
        KoraError::SubjectSoftDeleted(subject.to_string())
    } else {
        KoraError::SubjectNotFound
    }
}

/// Maximum allowed length for a subject name.
const MAX_SUBJECT_LENGTH: usize = 255;

/// Validate the subject path parameter.
fn validate_subject(subject: &str) -> Result<(), KoraError> {
    if subject.is_empty() {
        return Err(KoraError::InvalidSchema(
            "Subject name must not be empty".into(),
        ));
    }
    if subject.len() > MAX_SUBJECT_LENGTH {
        return Err(KoraError::InvalidSchema(
            "Subject name exceeds maximum length".into(),
        ));
    }
    if subject.contains('\0') {
        return Err(KoraError::InvalidSchema(
            "Subject name contains invalid characters".into(),
        ));
    }
    Ok(())
}

/// Parse a version string to a positive i32.
///
/// Confluent accepts only positive integers and `"latest"`. Everything else
/// (0, negatives, non-numeric strings) returns 42202 (`InvalidVersion`).
fn parse_version(version: &str) -> Result<i32, KoraError> {
    let v: i32 = version
        .parse()
        .map_err(|_| KoraError::InvalidVersion(version.to_string()))?;
    if v < 1 {
        return Err(KoraError::InvalidVersion(version.to_string()));
    }
    Ok(v)
}
