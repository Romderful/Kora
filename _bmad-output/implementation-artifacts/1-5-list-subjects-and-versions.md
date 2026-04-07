# Story 1.5: List Subjects and Versions

Status: done

## Story

As a **developer**,
I want to list all subjects and all versions of a subject,
so that I can discover available schemas in the registry.

## Acceptance Criteria

1. **Given** registered subjects "orders-value" and "users-value"
   **When** I send `GET /subjects`
   **Then** I receive HTTP 200 with `["orders-value", "users-value"]`

2. **Given** no registered subjects
   **When** I send `GET /subjects`
   **Then** I receive HTTP 200 with `[]`

3. **Given** subject "orders-value" with versions 1, 2, 3
   **When** I send `GET /subjects/orders-value/versions`
   **Then** I receive HTTP 200 with `[1, 2, 3]`

**FRs Covered:** FR6, FR7

## Tasks / Subtasks

- [x] Task 1: Storage layer — list subjects and versions (AC: #1, #2, #3)
  - [x] Add `subjects::list(pool) -> Result<Vec<String>, sqlx::Error>` in `src/storage/subjects.rs`
  - [x] SQL: `SELECT name FROM subjects WHERE deleted = false ORDER BY name`
  - [x] Add `schemas::list_versions(pool, subject) -> Result<Vec<i32>, sqlx::Error>` in `src/storage/schemas.rs`
  - [x] SQL: `SELECT s.version FROM schemas s JOIN subjects sub ON s.subject_id = sub.id WHERE sub.name = $1 AND s.deleted = false ORDER BY s.version`

- [x] Task 2: API handlers — GET /subjects and GET /subjects/{subject}/versions (AC: #1, #2, #3)
  - [x] Add `list_subjects` handler in `src/api/subjects.rs` — returns `Json(Vec<String>)`
  - [x] Add `list_versions` handler in `src/api/subjects.rs` — returns `Json(Vec<i32>)`
  - [x] `list_versions`: validate subject, check subject exists → `SubjectNotFound` (40401) if missing
  - [x] Register routes in `src/api/mod.rs`:
    - `GET /subjects` → `list_subjects`
    - `GET /subjects/{subject}/versions` coexists with `POST` via `get().post()` chain

- [x] Task 3: Integration tests (AC: #1, #2, #3)
  - [x] Create `tests/api_list_subjects.rs`
  - [x] Test: register schemas under 2 subjects, GET /subjects → 200 with both names (AC #1)
  - [x] Test: empty registry, GET /subjects → 200 with `[]` (AC #2)
  - [x] Test: register 2 versions, GET /subjects/{subject}/versions → 200 with `[1, 2]` (AC #3)
  - [x] Test: GET /subjects/nonexistent/versions → 404 + 40401

- [x] Task 4: Verify all tests pass
  - [x] `just lint` — zero warnings
  - [x] `just test` — 43/43 tests pass (39 existing + 4 new)

## Dev Notes

### Confluent API Contract

```
GET /subjects
Accept: application/vnd.schemaregistry.v1+json

Response (200): ["orders-value", "users-value"]
Response (200): []   (empty registry)
```

```
GET /subjects/{subject}/versions
Accept: application/vnd.schemaregistry.v1+json

Response (200): [1, 2, 3]
Response (404): {"error_code": 40401, "message": "Subject not found"}
```

Both endpoints return plain JSON arrays — no wrapper object.

### Soft-Delete Behavior

Both list endpoints respect soft-delete (`WHERE deleted = false`), consistent with story 1.4. Soft-deleted subjects/schemas are excluded from listings.

### Route Coexistence

`GET /subjects/{subject}/versions` (list versions) and `POST /subjects/{subject}/versions` (register schema) share the same path but differ by HTTP method. In axum, use `.route()` with `get(list_versions).post(register_schema)` on the same path.

### Architecture Compliance

- **Handler pattern**: `async fn handler(State(pool)) -> Result<impl IntoResponse, KoraError>`
- **Storage pattern**: Standalone async fns taking `&PgPool`. Use `sqlx::query_scalar` for single-column results (names, versions).
- **File placement**: Handlers in `src/api/subjects.rs`. Storage in `src/storage/subjects.rs` and `src/storage/schemas.rs`.
- **Response format**: Plain JSON arrays via `Json(vec)` — no wrapper struct needed.
- **No `unwrap()`** outside tests. `deny(missing_docs)` + `deny(clippy::pedantic)` enforced.

### Previous Story Intelligence (from 1.4)

- `SubjectNotFound` (40401) already exists in `KoraError` — reuse for list_versions on non-existent subject.
- `subjects::exists()` already exists — reuse for subject check in list_versions.
- `validate_subject()` already exists — reuse for subject validation.
- Storage uses `sqlx::query_scalar::<_, T>()` for single-value queries, `sqlx::query()` + `Row` for multi-column.
- Tests use `common::spawn_server()` helper. Convention: one test file per endpoint group.
- 39 tests currently across 8 test files.

### References

- [Source: epics.md — Epic 1, Story 1.5]
- [Source: prd.md — FR6, FR7]
- [Source: architecture.md — API patterns, subjects.rs handler]
- [Source: 1-4-retrieve-schema-by-subject-version.md — Completion Notes]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (1M context)

### Debug Log References

None required.

### Completion Notes List

- Added `subjects::list()` and `schemas::list_versions()` storage functions
- Added `list_subjects` and `list_versions` handlers
- Route coexistence: `get(list_versions).post(register_schema)` on same path — removed unused `post` import
- Both queries respect soft-delete (`WHERE deleted = false`)
- 4 new integration tests, 43 total, all passing
- Review: clean, all ACs verified

### File List

**New:**
- `tests/api_list_subjects.rs`

**Modified:**
- `src/api/mod.rs` — added routes, removed unused `post` import
- `src/api/subjects.rs` — added `list_subjects`, `list_versions` handlers
- `src/storage/subjects.rs` — added `list()`
- `src/storage/schemas.rs` — added `list_versions()`
