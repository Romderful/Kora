# Story 4.4: List All Schemas and Raw Schema Text Endpoints

Status: done

**Depends on:** Story 4.2 (pagination infrastructure), Story 4.3 (query param patterns, global dedup model)

## Story

As a **developer using Confluent-compatible tooling**,
I want to list all schemas globally and retrieve raw schema text,
so that I can discover and access schemas using all Confluent-compatible endpoints.

## Acceptance Criteria

### AC1: List All Schemas — Basic

**Given** registered schemas across multiple subjects
**When** I send `GET /schemas`
**Then** I receive HTTP 200 with a JSON array of schema objects, each containing `subject`, `version`, `id`, `schema`, and optionally `schemaType` (omitted for AVRO) and `references` (omitted when empty)

### AC2: List All Schemas — `subjectPrefix` Filter

**Given** registered schemas and query parameter `subjectPrefix=orders`
**When** I send `GET /schemas?subjectPrefix=orders`
**Then** I receive only schemas whose subject starts with "orders"

**Given** `GET /schemas` with no `subjectPrefix` param (or `subjectPrefix=:*:`)
**Then** all schemas are returned (default `:*:` = match all)

### AC3: List All Schemas — `deleted`, `latestOnly`, Pagination

**Given** query parameters `deleted=true`, `latestOnly=true`, `offset=10`, `limit=20`
**When** I send `GET /schemas?deleted=true&latestOnly=true&offset=10&limit=20`
**Then** results include soft-deleted schemas, only latest version per subject, paginated accordingly

**Given** `latestOnly=true`
**Then** only the highest non-deleted version per subject is returned (or highest including deleted if `deleted=true`)

**Given** `deleted=true` without `latestOnly`
**Then** all versions are returned, including soft-deleted ones

### AC4: Raw Schema Text by Global ID

**Given** a registered schema with ID 1
**When** I send `GET /schemas/ids/1/schema`
**Then** I receive HTTP 200 with the raw schema text only (no wrapper object — just the schema string as a JSON value)

**Given** an optional `subject` query parameter
**When** I send `GET /schemas/ids/1/schema?subject=orders-value`
**Then** the `subject` param is accepted (used for schema resolution context in future, accept-and-ignore for now)

**Given** a non-existent schema ID
**When** I send `GET /schemas/ids/999/schema`
**Then** I receive HTTP 404 with error code 40403

### AC5: Raw Schema Text by Subject/Version

**Given** subject "orders-value" with version 2
**When** I send `GET /subjects/orders-value/versions/2/schema`
**Then** I receive HTTP 200 with the raw schema text only (no wrapper object)

**Given** a soft-deleted version with `deleted=true`
**When** I send `GET /subjects/orders-value/versions/2/schema?deleted=true`
**Then** the soft-deleted version's schema text is returned

**Given** a soft-deleted version without `deleted` param
**When** I send `GET /subjects/orders-value/versions/2/schema`
**Then** I receive HTTP 404 with error code 40402

**Given** `GET /subjects/orders-value/versions/latest/schema`
**Then** the latest version's raw schema text is returned

**Given** a non-existent subject
**When** I send `GET /subjects/unknown/versions/1/schema`
**Then** I receive HTTP 404 with error code 40401

**FRs:** FR33, FR34, FR35

## Tasks / Subtasks

- [x] Task 1: `GET /schemas` — list all schemas endpoint (AC: 1, 2, 3)
  - [x] 1.1: Create `ListSchemasParams` query param struct in `src/api/schemas.rs`
    - `deleted: bool` (default false) — include soft-deleted versions
    - `latestOnly: bool` (serde rename, default false) — only highest version per subject
    - `subjectPrefix: String` (serde rename, default `:*:`) — filter by prefix
    - `offset: i64` (default 0) — pagination offset
    - `limit: i64` (default -1) — pagination limit
  - [x] 1.2: Create `list_all_schemas` storage function in `src/storage/schemas.rs`
    - JOIN `schema_versions sv` → `subjects sub` → `schema_contents sc`
    - Filter: `(sv.deleted = false OR $include_deleted)` AND `(sub.deleted = false OR $include_deleted)`
    - Filter: subject prefix via LIKE with backslash escaping (reuse pattern from `storage::subjects::list_subjects`)
    - `latestOnly=true`: use `DISTINCT ON (sub.name)` with `ORDER BY sub.name, sv.version DESC`
    - Pagination: `OFFSET $n`, `LIMIT $n` only when limit >= 0
    - Return: `Vec<SchemaVersion>` (reuse existing struct — already has subject, id, version, schema, schema_type, references)
  - [x] 1.3: Create `list_all_schemas` handler in `src/api/schemas.rs`
    - Deserialize `ListSchemasParams` from query
    - Call storage function
    - Populate references for each returned schema via `references::find_references_by_schema_id`
    - Return `Json(schemas)`
  - [x] 1.4: Register route `GET /schemas` in `src/api/mod.rs`
    - Add `.route("/schemas", get(schemas::list_all_schemas))` — must be BEFORE `/schemas/ids/{id}` to avoid route conflicts
  - [x] 1.5: Tests — distribute into existing test modules or a new focused module
    - List returns all schemas with correct fields
    - `subjectPrefix` filters correctly (including LIKE metachar escaping)
    - `latestOnly=true` returns only highest version per subject
    - `deleted=true` includes soft-deleted schemas
    - `deleted=true` + `latestOnly=true` combination
    - Pagination (offset + limit)
    - Empty registry returns `[]`
    - `schemaType` omitted for AVRO, present for JSON/PROTOBUF

- [x] Task 2: `GET /schemas/ids/{id}/schema` — raw text by global ID (AC: 4)
  - [x] 2.1: Create `GetRawSchemaParams` query param struct in `src/api/schemas.rs`
    - `subject: Option<String>` — accept and ignore (future: schema resolution context)
  - [x] 2.2: Create `get_raw_schema_by_id` handler in `src/api/schemas.rs`
    - Reuse `schemas::find_schema_by_id(&pool, id)` — already returns `(schema_text, schema_type)`
    - Return raw schema text as JSON string: `Json(schema_text)`
    - On not found: return `KoraError::SchemaNotFound` (40403)
  - [x] 2.3: Register route in `src/api/mod.rs`
    - Add `.route("/schemas/ids/{id}/schema", get(schemas::get_raw_schema_by_id))`
  - [x] 2.4: Tests
    - Returns raw text for AVRO schema (valid JSON)
    - Returns raw text for JSON Schema
    - Returns raw text for Protobuf (plain text as JSON string)
    - Non-existent ID → 404 / 40403
    - `subject` param accepted without error

- [x] Task 3: `GET /subjects/{subject}/versions/{version}/schema` — raw text by version (AC: 5)
  - [x] 3.1: Create `get_raw_schema_by_version` handler in `src/api/subjects.rs`
    - Reuse `GetVersionParams` (already has `deleted: bool`)
    - Parse version string (reuse `parse_version` helper, support "latest")
    - Call `schemas::find_schema_by_subject_version` or `find_latest_schema_by_subject`
    - Return raw schema text as JSON string: `Json(sv.schema)`
    - Error handling: same pattern as `get_schema_by_version` — `SubjectNotFound` / `VersionNotFound`
  - [x] 3.2: Register route in `src/api/mod.rs`
    - Add `.route("/subjects/{subject}/versions/{version}/schema", get(subjects::get_raw_schema_by_version))`
  - [x] 3.3: Tests
    - Returns raw text for registered schema
    - `deleted=true` returns soft-deleted version's text
    - Without `deleted` param, soft-deleted version → 404 / 40402
    - `latest` version string works
    - Non-existent subject → 404 / 40401
    - Non-existent version → 404 / 40402
    - Invalid version (0, negative) → 422 / 42202

## Dev Notes

### Architecture Compliance

- **No new modules** — handlers go in existing `src/api/schemas.rs` and `src/api/subjects.rs`
- **Storage function** — add `list_all_schemas` to `src/storage/schemas.rs` (follows existing pattern of one concern per function)
- **Reuse `SchemaVersion` struct** for `GET /schemas` response — it already serializes with `schemaType` skip for AVRO and `references` skip when empty
- **Reuse `find_schema_by_id`** for raw-by-ID — it already returns just `(schema_text, schema_type)`, only need `schema_text`
- **Reuse existing helpers** — `parse_version`, `validate_subject`, `default_limit`, `default_subject_prefix`, `load_references`

### Raw Schema Text Response Format

Confluent's `/schema` endpoints return the raw schema string as a JSON-encoded string value. In axum:

```rust
// For all schema types: return schema_text as a JSON string value
Ok(Json(schema_text))  // axum serializes String to a JSON string
```

This produces: `"{"type":"record","name":"test","fields":[]}"` for Avro/JSON, or `"syntax = \"proto3\"; ..."` for Protobuf. The content type remains `application/vnd.schemaregistry.v1+json` (handled by existing middleware).

### `GET /schemas` SQL Pattern

The core query needs a 3-way JOIN. Two variants for `latestOnly`:

**All versions (latestOnly=false):**
```sql
SELECT sub.name AS subject, sc.id, sv.version,
       sc.schema_text AS schema, sc.schema_type
FROM schema_versions sv
JOIN subjects sub ON sv.subject_id = sub.id
JOIN schema_contents sc ON sv.content_id = sc.id
WHERE (sv.deleted = false OR $1)
  AND (sub.deleted = false OR $1)
  AND (sub.name LIKE $2 ESCAPE '\' OR $2 = ':*:')
ORDER BY sub.name, sv.version
OFFSET $3
-- LIMIT $4 (only when >= 0)
```

**Latest only (latestOnly=true):**
```sql
SELECT DISTINCT ON (sub.name)
       sub.name AS subject, sc.id, sv.version,
       sc.schema_text AS schema, sc.schema_type
FROM schema_versions sv
JOIN subjects sub ON sv.subject_id = sub.id
JOIN schema_contents sc ON sv.content_id = sc.id
WHERE (sv.deleted = false OR $1)
  AND (sub.deleted = false OR $1)
  AND (sub.name LIKE $2 ESCAPE '\' OR $2 = ':*:')
ORDER BY sub.name, sv.version DESC
OFFSET $3
-- LIMIT $4 (only when >= 0)
```

**LIKE metacharacter escaping** — reuse the same pattern from `storage::subjects::list_subjects`: escape `%`, `_`, `\` with `\` before building the `prefix%` pattern. When prefix is `:*:` or empty, skip the LIKE filter entirely.

### Pagination Pattern

Follow the established match-on-limit pattern from cross-reference queries:
```rust
match limit >= 0 {
    true => sqlx::query_as(SQL_WITH_LIMIT).bind(...).bind(limit).fetch_all(pool).await?,
    false => sqlx::query_as(SQL_WITHOUT_LIMIT).bind(...).fetch_all(pool).await?,
}
```

### Route Registration Order

The `/schemas` route MUST be registered BEFORE `/schemas/ids/{id}` in axum to avoid the `{id}` path segment matching the literal "ids". Similarly, `/schemas/ids/{id}/schema` must be a separate route from `/schemas/ids/{id}`. Check that axum resolves these correctly — they should be fine since axum uses a trie-based router, but verify with tests.

### References Population for `GET /schemas`

Each schema in the list may have references. Two strategies:
1. **N+1 approach**: For each schema in the list, call `references::find_references_by_schema_id`. Simple, matches existing patterns.
2. **Batch approach**: Single query to fetch all references for the returned content IDs, then map them in Rust.

Start with approach 1 (consistent with existing code). Optimize to batch if performance becomes an issue — the `GET /schemas` endpoint is not on the hot path (admin/discovery use only, not called by serializers/deserializers).

### Files to Modify

| File | Changes |
|------|---------|
| `src/api/schemas.rs` | Add `ListSchemasParams`, `GetRawSchemaParams` structs; add `list_all_schemas`, `get_raw_schema_by_id` handlers |
| `src/api/subjects.rs` | Add `get_raw_schema_by_version` handler |
| `src/api/mod.rs` | Register 3 new routes: `/schemas`, `/schemas/ids/{id}/schema`, `/subjects/{subject}/versions/{version}/schema` |
| `src/storage/schemas.rs` | Add `list_all_schemas` function (3-way JOIN, prefix filter, latestOnly, deleted, pagination) |

### Testing Standards

- **TDD**: Write failing tests first, then implement
- **Integration tests** against the HTTP API (spawn_server pattern)
- **Distribute tests** into existing test modules — do NOT create standalone test files unless clearly justified
- Use `unique_avro_schema()` helper for content isolation between tests
- Use UUID-based subject names to avoid cross-test interference
- Verify HTTP status codes AND Confluent error codes in error cases

### Project Structure Notes

- All new code goes into existing files — no new modules or files needed (except potentially tests)
- `SchemaVersion` struct in `storage/schemas.rs` is reusable as-is for `GET /schemas` response items
- The `is_avro` serde skip function and `references` skip-when-empty already handle Confluent format quirks
- `default_limit()` and `default_subject_prefix()` are already defined and reusable

### Previous Story Intelligence

From Story 4.3 implementation:
- **Global dedup model**: `schema_contents` + `schema_versions` two-table design is complete. Same schema under different subjects shares one content ID. All queries must JOIN through both tables.
- **LIKE escaping**: Subject prefix filtering requires escaping `%`, `_`, `\` metacharacters. Pattern established in `storage::subjects::list_subjects`.
- **`schemaType` omission**: For AVRO schemas, `schemaType` is omitted in responses. `SchemaVersion` struct handles this via `#[serde(skip_serializing_if = "is_avro")]`.
- **Normalize infrastructure**: Config-driven normalize with subject/global fallback exists but is NOT relevant to this story (list/raw endpoints don't need normalize).
- **`raw_fingerprint` column**: Added for normalize=false comparisons. Not relevant to this story's endpoints.
- **Query param struct pattern**: Use `#[serde(default, rename = "camelCase")]` for Confluent params. Use `default_limit()` const fn for -1 default.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 4.4] — AC definitions
- [Source: _bmad-output/planning-artifacts/prd.md#FR33-FR35] — Functional requirements
- [Source: _bmad-output/planning-artifacts/architecture.md#API Patterns] — Confluent format, error mapping, module organization
- [Source: src/api/schemas.rs] — Existing handler patterns, `CrossRefParams`, `GetSchemaByIdParams`
- [Source: src/storage/schemas.rs] — `find_schema_by_id`, `SchemaVersion` struct, 3-way JOIN patterns
- [Source: src/api/subjects.rs] — `parse_version`, `validate_subject`, `GetVersionParams`, `load_references`
- [Source: src/storage/subjects.rs] — LIKE prefix escaping pattern in `list_subjects`

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (1M context)

### Debug Log References

### Completion Notes List

- **Task 1**: Implemented `GET /schemas` endpoint with `ListSchemasParams` (deleted, latestOnly, subjectPrefix, offset, limit). Created `list_all_schemas` storage function with 3-way JOIN (schema_versions + subjects + schema_contents), LIKE prefix escaping, `DISTINCT ON` for latestOnly, and pagination. Split into `list_all_schemas_latest` and `list_all_schemas_all` helpers to satisfy clippy pedantic line limit. Handler populates references via N+1 pattern. 9 integration tests.
- **Task 2**: Implemented `GET /schemas/ids/{id}/schema` endpoint. Reuses `find_schema_by_id` storage function. Returns raw schema text as JSON string. `GetRawSchemaParams` accepts `subject` (accept-and-ignore). 5 integration tests (AVRO, JSON, Protobuf, 404, subject param).
- **Task 3**: Implemented `GET /subjects/{subject}/versions/{version}/schema` endpoint. Reuses `GetVersionParams`, `parse_version`, `find_schema_by_subject_version`, `find_latest_schema_by_subject`. Returns `sv.schema` as JSON string. Same error handling pattern as `get_schema_by_version`. 7 integration tests (happy path, latest, soft-delete with/without param, 40401, 40402, 42202).

### Change Log

- 2026-04-13: Implemented all 3 tasks for story 4.4. Added 3 new endpoints: `GET /schemas`, `GET /schemas/ids/{id}/schema`, `GET /subjects/{subject}/versions/{version}/schema`. 27 new integration tests across 3 test files. Made `default_subject_prefix()` pub(crate) for reuse. All 172 tests pass, clippy pedantic clean.

### File List

- src/api/mod.rs (modified) — registered 3 new routes
- src/api/schemas.rs (modified) — added `ListSchemasParams`, `GetRawSchemaParams` structs; `list_all_schemas`, `get_raw_schema_by_id` handlers
- src/api/subjects.rs (modified) — added `get_raw_schema_by_version` handler; made `default_subject_prefix` pub(crate)
- src/storage/schemas.rs (modified) — added `list_all_schemas`, `list_all_schemas_latest`, `list_all_schemas_all` functions
- tests/api_get_schema_by_id.rs (modified) — +5 raw-schema-by-id tests
- tests/api_get_schema_by_version.rs (modified) — +7 raw-schema-by-version tests
- tests/api_list_schemas.rs (new) — 9 list-all-schemas tests (basic, prefix, latestOnly, deleted, pagination, schemaType, LIKE escaping)
