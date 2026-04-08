# Story 2.2: Hard-Delete Subject and Versions

Status: done

## Story

As a **developer**,
I want to permanently delete a subject or version,
so that I can completely remove schemas that should not exist.

## Acceptance Criteria

1. **Given** a soft-deleted subject "orders-value"
   **When** I send `DELETE /subjects/orders-value?permanent=true`
   **Then** I receive HTTP 200 with the list of permanently deleted versions
   **And** the schema data is removed from PostgreSQL

2. **Given** a soft-deleted version 2 of subject "orders-value"
   **When** I send `DELETE /subjects/orders-value/versions/2?permanent=true`
   **Then** I receive HTTP 200 with `2`

3. **Given** a subject that is NOT soft-deleted
   **When** I send `DELETE /subjects/orders-value?permanent=true`
   **Then** I receive HTTP 404 with Confluent error code 40401

**FRs Covered:** FR10, FR11

## Tasks / Subtasks

- [x] Task 1: Storage layer — hard-delete operations (AC: #1, #2)
  - [x] Add `subjects::hard_delete(pool, name) -> Result<Vec<i32>, sqlx::Error>` — DELETE FROM schemas + subjects WHERE deleted = true, returns version numbers
  - [x] Add `schemas::hard_delete_version(pool, subject, version) -> Result<Option<i32>, sqlx::Error>` — DELETE FROM schemas WHERE deleted = true AND version = $1

- [x] Task 2: Update DELETE handlers to support `?permanent=true` (AC: #1, #2, #3)
  - [x] Add `permanent` field to `DeletedParam` or create new `DeleteParam`
  - [x] In `delete_subject`: if permanent=true, check subject is soft-deleted (deleted=true in DB), then hard-delete
  - [x] In `delete_version`: if permanent=true, check version is soft-deleted, then hard-delete
  - [x] Non-soft-deleted subject with permanent=true → 40401 (AC #3)

- [x] Task 3: Integration tests (AC: #1, #2, #3)
  - [x] Create `tests/api_hard_delete.rs`
  - [x] Test: soft-delete then hard-delete subject → 200 + versions, data gone from DB (AC #1)
  - [x] Test: soft-delete version then hard-delete version → 200 + version number (AC #2)
  - [x] Test: hard-delete non-soft-deleted subject → 404 + 40401 (AC #3)
  - [x] Test: hard-delete non-soft-deleted version → 404 + 40402
  - [x] Test: hard-delete already hard-deleted subject → 404 + 40401

- [x] Task 4: Verify all tests pass
  - [x] `just lint` — zero warnings
  - [x] `just test` — all tests pass (56 existing + new)

## Dev Notes

### Confluent API Contract

```
DELETE /subjects/{subject}?permanent=true
Response (200): [1, 2, 3]   (permanently deleted versions)
Response (404): {"error_code": 40401, "message": "Subject not found"}

DELETE /subjects/{subject}/versions/{version}?permanent=true
Response (200): 2   (the version number)
Response (404): {"error_code": 40402, "message": "Version not found"}
```

### Two-Step Delete (Confluent/Karapace behavior)

Hard delete requires prior soft-delete. The flow is:
1. `DELETE /subjects/{subject}` → soft-delete (marks deleted=true)
2. `DELETE /subjects/{subject}?permanent=true` → hard-delete (removes from DB)

Attempting hard-delete on a non-soft-deleted subject returns 40401.

### Existing exists() behavior

`exists()` filters `deleted = false`. For hard-delete, we need to check for soft-deleted subjects/versions (deleted = true). Don't reuse `exists()` — query directly for `deleted = true`.

### Architecture Compliance

- Same handler functions (`delete_subject`, `delete_version`) — branch on `permanent` param
- SQL: `DELETE FROM schemas WHERE ...` and `DELETE FROM subjects WHERE ...`
- Transaction for subject hard-delete (delete schemas first, then subject)
- Response format identical to soft-delete (version list or single int)

### References

- [Source: epics.md — Epic 2, Story 2.2]
- [Source: prd.md — FR10, FR11]
- [Source: Karapace — two-step delete requirement]
- [Source: 2-1-soft-delete-subject-and-versions.md — Completion Notes]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (1M context)

### Completion Notes List

- Added `subjects::hard_delete()` — transaction-wrapped DELETE FROM, returns sorted versions
- Added `schemas::hard_delete_version()` — DELETE FROM single version where deleted=true
- Added `PermanentParam` query struct for `?permanent=true`
- Updated `delete_subject` and `delete_version` handlers to branch on permanent param
- Two-step delete enforced: hard-delete only operates on deleted=true rows (Confluent/Karapace)
- Extracted `parse_version()` helper, reused by get_schema_by_version and delete_version
- Centralized test helpers in `tests/common/api.rs` (8 helpers) + fixtures in `tests/common/mod.rs`
- Added `INCLUDE_DELETED`/`ACTIVE_ONLY` constants for explicit bool params
- Removed all inline HTTP duplication and direct SQL setup from tests
- Removed useless test `list_subjects_returns_valid_array`
- 60 tests total (5 new for hard-delete), all passing
- Review: CLEAN

### File List

**New:**
- `tests/api_hard_delete.rs`
- `tests/common/api.rs`

**Modified:**
- `src/api/subjects.rs` — added PermanentParam, updated delete handlers, extracted parse_version
- `src/storage/subjects.rs` — added hard_delete()
- `src/storage/schemas.rs` — added hard_delete_version()
- `tests/common/mod.rs` — added fixtures, constants, sections
- `tests/api_check_schema.rs` — use common helpers
- `tests/api_delete_subject.rs` — use common helpers
- `tests/api_get_schema_by_id.rs` — use common helpers
- `tests/api_get_schema_by_version.rs` — use common helpers
- `tests/api_list_subjects.rs` — use common helpers, removed useless test
- `tests/api_register_schema.rs` — use common helpers
