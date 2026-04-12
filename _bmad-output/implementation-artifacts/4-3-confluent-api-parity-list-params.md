# Story 4.3: Confluent API Parity — List & Lookup Params

Status: done

**Depends on:** Story 4.2 (pagination infrastructure, error codes)

## Story

As a **developer using Confluent-compatible tooling**,
I want all list and lookup endpoints to support the full set of Confluent query parameters (subjectPrefix, deletedOnly, deletedAsNegative, normalize, deleted on cross-refs and check),
so that every Confluent client library query works identically against Kora.

## Acceptance Criteria

### AC1: List Subjects — Full Param Support

**Given** `GET /subjects` with no `subjectPrefix` param
**Then** all subjects are returned (default `subjectPrefix` = `:*:` = match all)

**Given** `GET /subjects?subjectPrefix=orders`
**Then** only subjects whose name starts with "orders" are returned

**Given** `GET /subjects?deletedOnly=true`
**Then** ONLY soft-deleted subjects are returned (not active ones)

**Given** `GET /subjects?deleted=true&deletedOnly=true`
**Then** `deletedOnly` takes precedence — only soft-deleted subjects returned

### AC2: List Versions — Full Param Support

**Given** `GET /subjects/{subject}/versions?deletedOnly=true`
**Then** only soft-deleted versions are returned

**Given** `GET /subjects/{subject}/versions?deletedAsNegative=true`
**Then** soft-deleted versions appear as negative numbers (e.g., version 2 soft-deleted → -2 in the list)
**And** active versions appear as positive numbers

### AC3: Register Schema — `normalize` Param

**Given** `POST /subjects/{subject}/versions?normalize=true`
**When** I register a schema
**Then** the schema is normalized (canonical_form) before fingerprint comparison and storage deduplication

**Given** two schemas that differ only in whitespace/ordering
**When** both registered with `normalize=true`
**Then** the second registration returns the same ID (deduplicated via canonical form)

### AC4: Check Schema — Extended Params

**Given** `POST /subjects/{subject}?normalize=true`
**When** checking if a schema is registered
**Then** the lookup uses the normalized (canonical) form for comparison

**Given** `POST /subjects/{subject}?deleted=true`
**Then** soft-deleted schema matches are included in the lookup result

### AC5: Cross-References — `deleted` + `subject` Params

**Given** `GET /schemas/ids/{id}/subjects?deleted=true`
**Then** soft-deleted subjects are included in the result

**Given** `GET /schemas/ids/{id}/versions?deleted=true`
**Then** soft-deleted versions are included in the result

**Given** `GET /schemas/ids/{id}/subjects?subject=orders-value`
**Then** results are filtered to only the "orders-value" subject

**Given** `GET /schemas/ids/{id}/versions?subject=orders-value`
**Then** results are filtered to versions under "orders-value" only

### AC6: Get Schema by Version — `deleted` Param

**Given** a soft-deleted version 2 of subject "orders-value"
**When** I send `GET /subjects/orders-value/versions/2?deleted=true`
**Then** the soft-deleted version is returned

**Given** a soft-deleted version 2 and no `deleted` param
**When** I send `GET /subjects/orders-value/versions/2`
**Then** I receive HTTP 404 with error code 40402

### AC7: Referencedby — `deleted` Param

**Given** `GET /subjects/{subject}/versions/{version}/referencedby?deleted=true`
**Then** soft-deleted referencing schemas are included

**Note:** referencedby endpoint is from story 4.5 (not yet implemented). This AC applies once 4.5 is done.

## Tasks / Subtasks

- [x] Task 1: `subjectPrefix` + `deletedOnly` on list subjects (AC: 1)
  - [x] Extend `ListParams` into `ListSubjectsParams { deleted: bool, deleted_only: bool, lookup_deleted_subject: bool, subject_prefix: String, offset: i64, limit: i64 }` with serde renames (`deletedOnly`, `lookupDeletedSubject`, `subjectPrefix`)
  - [x] `lookupDeletedSubject`: accept-and-ignore (Confluent OSS has it, overlaps with `deleted`/`deletedOnly`)
  - [x] Default `subject_prefix` to `":*:"` via custom serde default
  - [x] Extend `storage::list_subjects` to accept prefix + deleted_only
  - [x] SQL: add `WHERE name LIKE $prefix%` (skip when prefix is `:*:`), add `WHERE deleted = true` for deletedOnly
  - [x] Tests: prefix filtering, deletedOnly, default behavior

- [x] Task 2: `deletedOnly` + `deletedAsNegative` on list versions (AC: 2)
  - [x] Extend `ListParams` into `ListVersionsParams { deleted: bool, deleted_only: bool, deleted_as_negative: bool, offset: i64, limit: i64 }` with serde renames
  - [x] Extend `storage::list_schema_versions` to support `deleted_only` and `deleted_as_negative`
  - [x] For `deletedAsNegative`: SQL returns `-version` for deleted rows, `version` for active rows, ordered by `abs(version)`
  - [x] Tests: deletedOnly, deletedAsNegative with mix of active/deleted versions

- [x] Task 3: `normalize` on register + check (AC: 3, 4)
  - [x] Add `normalize: bool` query param to `register_schema` and `check_schema` handlers
  - [x] When `normalize=true` on register: compare fingerprints using canonical_form (already stored) instead of raw schema text
  - [x] When `normalize=true` on check: same — use canonical form for lookup
  - [x] Tests: register two formatting-different schemas with normalize=true → same ID; check with normalize=true finds match

- [x] Task 4: `deleted` param on check_schema (AC: 4)
  - [x] Add `deleted: bool` to check_schema handler query params
  - [x] When `deleted=true`: include soft-deleted schemas in lookup
  - [x] Modify `storage::find_schema_by_subject_id_and_fingerprint` to accept `include_deleted`
  - [x] Tests: check against soft-deleted schema with deleted=true returns match

- [x] Task 5: `deleted` + `subject` params on cross-references (AC: 5)
  - [x] Add `deleted: bool` and `subject: Option<String>` to `get_subjects_by_schema_id` and `get_versions_by_schema_id` handlers
  - [x] `subject` param filters results to a specific subject name (Confluent uses this to scope cross-reference lookups)
  - [x] Modify `storage::find_subjects_by_schema_id` and `find_versions_by_schema_id` to accept `include_deleted` and optional `subject` filter
  - [x] Tests: cross-refs include soft-deleted when deleted=true; subject filter narrows results

- [x] Task 6: `deleted` param on get-by-version (AC: 6)
  - [x] Add `deleted: bool` query param to `get_schema_by_version` handler
  - [x] Modify `storage::find_schema_by_subject_version` to accept `include_deleted`
  - [x] Tests: get soft-deleted version with deleted=true → 200; without → 404

## Dev Notes

### Architecture Compliance

- **No new routes** — all changes are query param additions to existing handlers
- **Extend existing `ListParams`** into richer param structs per handler
- **Storage layer**: add boolean/string params to existing functions — do NOT create new functions
- **Pagination** from story 4.2 is already wired — compose with new param structs via `#[serde(flatten)]` or explicit fields

### Query Param Struct Pattern

```rust
#[derive(Debug, Deserialize)]
pub struct ListSubjectsParams {
    #[serde(default)]
    pub deleted: bool,
    #[serde(default, rename = "deletedOnly")]
    pub deleted_only: bool,
    #[serde(default = "default_subject_prefix", rename = "subjectPrefix")]
    pub subject_prefix: String,
    #[serde(default)]
    pub offset: i64,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_subject_prefix() -> String { ":*:".to_string() }
fn default_limit() -> i64 { -1 }
```

### `normalize` Implementation

The `canonical_form` column is already populated for every schema at registration time. When `normalize=true`:
- **Register**: compute fingerprint from `canonical_form` instead of raw `schema_text` for dedup check
- **Check/Lookup**: same — match against canonical fingerprint
- Current code in `subjects.rs` uses `schema::parse()` which returns `ParsedSchema { canonical_form, fingerprint }`. The fingerprint is already based on canonical form. So `normalize=true` may already be the default behavior — **verify before implementing**. If fingerprint is already canonical-based, `normalize` is a no-op (accept the param, no behavior change).

### `deletedAsNegative` SQL Pattern

```sql
SELECT CASE WHEN s.deleted THEN -s.version ELSE s.version END as version
FROM schemas s JOIN subjects sub ON s.subject_id = sub.id
WHERE sub.name = $1
ORDER BY abs(s.version)
OFFSET $2
-- LIMIT $3 (omit when -1)
```

### Files to Modify

| File | Changes |
|------|---------|
| `src/api/subjects.rs` | Extend ListParams into richer structs, add normalize/deleted params |
| `src/api/schemas.rs` | Add deleted param to cross-reference handlers |
| `src/storage/subjects.rs` | Add subjectPrefix, deletedOnly to list_subjects |
| `src/storage/schemas.rs` | Add deletedOnly, deletedAsNegative to list_versions; add include_deleted to cross-ref + version queries |
| `tests/api_confluent_parity_params.rs` | New test file for list/lookup param ACs |
| `tests/common/api.rs` | Extend helpers with new param support |

### References

- [Source: Confluent Java — SubjectsResource.java] — `subjectPrefix` default `:*:`, `deletedOnly` param
- [Source: Confluent Java — SubjectVersionsResource.java] — `deletedAsNegative`, `deletedOnly` params
- [Source: _bmad-output/planning-artifacts/epics.md] — Stories 1.5, 1.6, 2.3 acceptance criteria

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (1M context)

### Completion Notes List

- **Task 1**: Created `ListSubjectsParams` with `subjectPrefix`, `deletedOnly`, `lookupDeletedSubject` (accept-and-ignore). Updated `storage::list_subjects` to support prefix LIKE filtering and deleted_only mode. Default prefix `:*:` and empty string both return all subjects. 6 tests.
- **Task 2**: Created `ListVersionsParams` with `deletedOnly` and `deletedAsNegative`. Updated `storage::list_schema_versions` with CASE WHEN SQL for negative version numbers and abs() ordering. 2 tests.
- **Task 3**: Added `RegisterParams` (normalize) and `CheckParams` (normalize + deleted) query param structs. Implemented full Confluent normalize behavior: `normalize=true` compares canonical fingerprint, `normalize=false` (default) compares raw text fingerprint (SHA-256). Added `raw_fingerprint` column to schemas table, `normalize` column to config table. Config-driven normalize: handler resolves `params.normalize OR get_effective_normalize(subject)` (subject-level then global fallback). 3 tests (normalize dedup, config-driven normalize, without-normalize creates separate versions).
- **Task 4**: Extended `check_schema` handler with `deleted` param. Added `find_subject_id_by_name_ext` to storage for deleted-aware subject lookup. Extended `find_schema_by_subject_id_and_fingerprint` with `include_deleted`. 1 test.
- **Task 5**: Extended `CrossRefParams` with `deleted` and `subject` fields. Updated `find_subjects_by_schema_id` and `find_versions_by_schema_id` storage functions with include_deleted and subject_filter params. 4 tests.
- **Task 6**: Added `GetVersionParams` with `deleted` field. Extended `find_schema_by_subject_version` with `include_deleted`. 1 test.

### Change Log

- 2026-04-12: Implemented all 6 tasks for Confluent API parity query parameters (AC1–AC6). Distributed integration tests across existing test modules. Removed dead ListParams struct.
- 2026-04-12: Post-review round 1 — propagated `deleted` to `find_latest_schema_by_subject` for `latest?deleted=true`; fixed `deletedOnly+deletedAsNegative` to return negative versions.
- 2026-04-12: Implemented full `normalize=false` raw text comparison (Confluent parity). Added `raw_fingerprint TEXT NOT NULL` column + indexes. Added `normalize BOOLEAN` to config table with `get_effective_normalize` (subject then global fallback). Config endpoint accepts/persists/returns `normalize`.
- 2026-04-12: Post-review rounds 2-4 — added indexes on `(subject_id, fingerprint)` and `(subject_id, raw_fingerprint)`; `list_versions` works on soft-deleted subjects with `deleted=true`; error propagation on `get_effective_normalize`; LIKE metacharacter escaping (`%`, `_`, `\`); `get_schema_by_version` correct error for soft-deleted subjects; partial unique index for global config NULL row; `SchemaVersion` omits `schemaType` for AVRO and includes `references`; PUT/DELETE config return `normalize`; lazy exists_check on `get_schema_by_version` happy path. Renamed `PermanentParam` → `PermanentParams`, `DefaultToGlobalParam` → `DefaultToGlobalParams` for consistency.
- 2026-04-12: Refactored schema parsers — parsers now return `(canonical_form, fingerprint)` tuple, `parse()` constructs the full `ParsedSchema` with `raw_fingerprint`. Eliminates incomplete intermediate state. Renamed `with_references` → `load_references`. Fixed flaky test timing in justfile (`db_ready` check after `pg_isready`).
- 2026-04-12: Unified storage functions — removed dead `find_subject_id_by_name` (active-only), renamed `_with_deleted` variant to `find_subject_id_by_name(pool, name, include_deleted)`. Merged `subject_exists` + `subject_exists_any` into `subject_exists(pool, name, include_deleted)`. 146 tests pass, clippy clean.

### File List

- migrations/001_initial_schema.sql (modified) — added `raw_fingerprint TEXT NOT NULL DEFAULT ''` to schemas, `normalize BOOLEAN NOT NULL DEFAULT false` to config, indexes on `(subject_id, fingerprint)` and `(subject_id, raw_fingerprint)`, partial unique index for global config NULL row
- src/schema/mod.rs (modified) — `ParsedSchema` with `raw_fingerprint`; `parse()` constructs full struct from parser tuple + SHA-256 of raw text
- src/schema/avro.rs (modified) — returns `(canonical_form, fingerprint)` tuple instead of `ParsedSchema`
- src/schema/json_schema.rs (modified) — same
- src/schema/protobuf.rs (modified) — same
- src/api/subjects.rs (modified) — new param structs: ListSubjectsParams, ListVersionsParams, RegisterParams, CheckParams, GetVersionParams; normalize resolution via config; `load_references` helper; lazy exists_check; renamed PermanentParams
- justfile (modified) — added `db_ready` check (psql SELECT 1) after `pg_isready` to fix flaky test timing
- src/api/schemas.rs (modified) — extended CrossRefParams with deleted + subject
- src/api/compatibility.rs (modified) — CompatibilityRequest accepts normalize; GET/PUT/DELETE return normalize; renamed DefaultToGlobalParams
- src/storage/schemas.rs (modified) — `raw_fingerprint` in NewSchema + register; normalize-aware dedup; SchemaVersion with references + skip schemaType for AVRO; all query functions extended with include_deleted/subject_filter/normalize
- src/storage/subjects.rs (modified) — list_subjects with prefix (LIKE escaped) + deleted_only; find_subject_id_by_name_ext
- src/storage/compatibility.rs (modified) — get_global_normalize, get_subject_normalize, get_effective_normalize; set functions accept normalize; delete functions return previous normalize
- tests/api_list_subjects.rs (modified) — +9 tests: subjectPrefix, deletedOnly, deletedAsNegative, combined, lookupDeletedSubject, soft-deleted subject with deleted=true
- tests/api_register_schema.rs (modified) — +4 tests: normalize dedup, config-driven normalize, JSON reorder with/without normalize
- tests/api_check_schema.rs (modified) — +3 tests: normalize match, without-normalize miss, deleted finds soft-deleted
- tests/api_schema_cross_refs.rs (modified) — +4 tests: deleted + subject filter on subjects and versions
- tests/api_get_schema_by_version.rs (modified) — +2 tests: deleted returns soft-deleted, latest?deleted=true; updated schemaType AVRO assertion
