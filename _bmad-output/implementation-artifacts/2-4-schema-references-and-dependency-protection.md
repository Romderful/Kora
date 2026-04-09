# Story 2.4: Schema References and Dependency Protection

Status: done

## Story

As a **developer**,
I want schemas to reference other schemas and be protected from deletion when referenced,
so that dependent schemas remain valid.

## Acceptance Criteria

1. **Given** a schema with `"references": [{"name": "User", "subject": "users-value", "version": 1}]`
   **When** I send `POST /subjects/orders-value/versions` with the referencing schema
   **Then** the system validates that "users-value" version 1 exists
   **And** stores the reference relationship in `schema_references`

2. **Given** a schema registration with a reference to a non-existent subject/version
   **When** I send `POST /subjects/orders-value/versions`
   **Then** I receive HTTP 422 with an error indicating the referenced schema was not found

3. **Given** schema "users-value" v1 is referenced by "orders-value" v1
   **When** I attempt to hard-delete "users-value" v1
   **Then** I receive HTTP 422 indicating the schema is referenced and cannot be deleted

**FRs Covered:** FR18, FR19

## Tasks / Subtasks

- [x] Task 1: Extend SchemaRequest to accept references (AC: #1)
  - [x] Add `references` field to `SchemaRequest` as `Option<Vec<SchemaReference>>`
  - [x] Add `SchemaReference` struct: `name: String, subject: String, version: i32`

- [x] Task 2: Storage layer — references module (AC: #1, #2, #3)
  - [x] Create `src/storage/references.rs`
  - [x] Add `insert(pool, schema_id, refs)` — insert into schema_references
  - [x] Add `validate_references(pool, refs)` — check each (subject, version) exists
  - [x] Add `is_version_referenced(pool, subject, version)` — check if referenced
  - [x] Register module in `src/storage/mod.rs`

- [x] Task 3: Add error variants (AC: #2, #3)
  - [x] Add `KoraError::ReferenceNotFound(String)` — 422/42201
  - [x] Add `KoraError::ReferenceExists(String)` — 422/42206

- [x] Task 4: Update register_schema handler (AC: #1, #2)
  - [x] Validate references before writes
  - [x] Store references after schema insert

- [x] Task 5: Update hard-delete handlers (AC: #3)
  - [x] delete_version: check is_version_referenced before hard-delete
  - [x] delete_subject: check each version for references before hard-delete

- [x] Task 6: Integration tests (AC: #1, #2, #3)
  - [x] Create `tests/api_schema_references.rs` (8 tests)
  - [x] Test: valid reference → 200 (AC #1)
  - [x] Test: no references → still works (backward compat)
  - [x] Test: non-existent subject reference → 422/42201 (AC #2)
  - [x] Test: non-existent version reference → 422/42201 (AC #2)
  - [x] Test: hard-delete referenced version → 422/42206 (AC #3)
  - [x] Test: hard-delete unreferenced version → 200
  - [x] Test: hard-delete subject with referenced version → 422/42206 (AC #3)
  - [x] Test: soft-delete referenced version → 200 (only hard-delete blocked)

- [x] Task 7: Verify all tests pass
  - [x] `cargo clippy` — zero warnings (pedantic)
  - [x] `cargo test` — 77 tests pass (69 existing + 8 new)

## Dev Notes

### Confluent API Contract — References

Registration request body with references:
```json
{
  "schema": "{...}",
  "schemaType": "AVRO",
  "references": [
    {"name": "User", "subject": "users-value", "version": 1}
  ]
}
```

Response: `{"id": <id>}` (same as without references)

Error when referenced schema not found:
```json
{"error_code": 42201, "message": "Invalid schema: reference not found ..."}
```

Error when trying to delete a referenced schema:
```json
{"error_code": 42206, "message": "One or more references exist to the schema ..."}
```

### Database — schema_references table (already exists)

```sql
CREATE TABLE IF NOT EXISTS schema_references (
    id        BIGSERIAL PRIMARY KEY,
    schema_id BIGINT NOT NULL REFERENCES schemas(id),
    name      TEXT NOT NULL,
    subject   TEXT NOT NULL,
    version   INT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_schema_references_schema_id ON schema_references(schema_id);
```

Note: no FK from (subject, version) to schemas — the validation is done at insert time in application code. This is consistent with Confluent's approach.

### Reference Validation Logic

For each reference in the request:
1. Look up the subject by name — must exist and not be soft-deleted
2. Look up the version under that subject — must exist and not be soft-deleted
3. If any reference fails validation → reject entire registration with 422/42201

### Deletion Protection Logic

On hard-delete of a version:
1. Get the schema_id for (subject, version)
2. Check if any row in `schema_references` points to this (subject, version) as a dependency
3. If referenced → reject with 422/42206

Important: the `schema_references` table stores `subject` (name) and `version` (int), NOT `schema_id` of the referenced schema. So the lookup is `SELECT EXISTS(SELECT 1 FROM schema_references WHERE subject = $1 AND version = $2)`.

### Existing Patterns

- **SchemaRequest**: `src/api/subjects.rs:18` — add `references` field here
- **register_schema handler**: `src/api/subjects.rs:53` — add validation and insert after line 79
- **hard_delete handlers**: `src/api/subjects.rs:192-246` — add reference check before delete
- **Error enum**: `src/error.rs` — add two new variants
- **Storage modules**: `src/storage/mod.rs` — add `pub mod references;`

### Transaction Requirement

The register_schema handler currently does upsert + find_by_fingerprint + insert as separate queries. With references, we need to wrap the insert + references insert in a transaction. The handler will need to take a transaction instead of the pool for the insert phase.

### Architecture Compliance

- New `storage/references.rs` module — follows one-file-per-concern pattern
- Handlers call storage directly — no service layer
- Error variants map to Confluent error codes
- Validation at API boundary — storage functions assume valid input
- TDD: tests first, code second

### References

- [Source: epics.md — Epic 2, Story 2.4]
- [Source: prd.md — FR18, FR19]
- [Source: architecture.md — storage/references.rs, error handling pattern]
- [Source: migrations/001_initial_schema.sql — schema_references table]
- [Source: 2-3-schema-id-cross-references.md — Completion Notes]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (1M context)

### Debug Log References

### Completion Notes List

- Added `SchemaReference` struct and `references` field to `SchemaRequest`
- Created `src/storage/references.rs` with validate_references, insert, is_version_referenced
- Added `KoraError::ReferenceNotFound` (42201) and `KoraError::ReferenceExists` (42206)
- Updated register_schema: validates references before writes, stores after insert
- Updated delete_subject/delete_version (permanent=true): checks references before hard-delete
- 8 integration tests covering all 3 ACs + backward compat + soft-delete still works
- 77 tests total, all passing, zero clippy warnings

### File List

**New:**
- `src/storage/references.rs`
- `tests/api_schema_references.rs`

**Modified:**
- `src/api/subjects.rs` — SchemaReference struct, references field, validation+insert in register, ref check in deletes
- `src/storage/mod.rs` — added references module
- `src/error.rs` — ReferenceNotFound, ReferenceExists variants

### Change Log

- 2026-04-09: Story 2.4 implemented — schema references and dependency protection (FR18, FR19)
- 2026-04-09: Code review fixes — clean up schema_references on hard-delete (FK safety), is_version_referenced filters deleted schemas, added chain-delete test
