# Story 4.1: Compatibility Configuration CRUD

Status: done

## Story

As a **developer**,
I want to get and set compatibility configuration at global and per-subject levels,
so that I can control how schema evolution is enforced.

## Acceptance Criteria

1. **Given** a running Kora server with default config
   **When** I send `GET /config`
   **Then** I receive HTTP 200 with `{"compatibilityLevel": "BACKWARD"}`

2. **Given** I want to change the global compatibility
   **When** I send `PUT /config` with `{"compatibility": "FULL"}`
   **Then** I receive HTTP 200 with `{"compatibility": "FULL"}`

3. **Given** subject "orders-value" exists
   **When** I send `PUT /config/orders-value` with `{"compatibility": "NONE"}`
   **Then** I receive HTTP 200 with `{"compatibility": "NONE"}`

4. **Given** subject "orders-value" has per-subject config
   **When** I send `GET /config/orders-value`
   **Then** I receive HTTP 200 with `{"compatibilityLevel": "NONE"}`

5. **Given** subject "orders-value" has per-subject config
   **When** I send `DELETE /config/orders-value`
   **Then** I receive HTTP 200 with `{"compatibility": "BACKWARD"}` (falls back to global)

**FRs Covered:** FR21, FR22, FR23, FR24, FR25

## Tasks / Subtasks

- [x] Task 1: Create compatibility storage module (AC: #1–#5)
  - [x] Create `src/storage/compatibility.rs`
  - [x] `get_global_level(pool) -> String` — reads config row where subject IS NULL
  - [x] `set_global_level(pool, level) -> String` — updates global config, returns new value
  - [x] `get_level(pool, subject) -> String` — per-subject with global fallback via `ORDER BY subject IS NULL`
  - [x] `set_subject_level(pool, subject, level) -> String` — upsert per-subject config
  - [x] `delete_subject_level(pool, subject) -> String` — deletes per-subject row, returns global fallback

- [x] Task 2: Create compatibility API handlers (AC: #1–#5)
  - [x] Create `src/api/compatibility.rs`
  - [x] `get_global_compatibility` — `GET /config` → `{"compatibilityLevel": "<level>"}`
  - [x] `set_global_compatibility` — `PUT /config` → `{"compatibility": "<level>"}`
  - [x] `get_subject_compatibility` — `GET /config/{subject}` → `{"compatibilityLevel": "<level>"}`
  - [x] `set_subject_compatibility` — `PUT /config/{subject}` → `{"compatibility": "<level>"}`
  - [x] `delete_subject_compatibility` — `DELETE /config/{subject}` → `{"compatibility": "<fallback>"}`
  - [x] Validate level against `COMPATIBILITY_LEVELS` constant (7 valid values)
  - [x] Return 404 (40401) for per-subject endpoints when subject doesn't exist

- [x] Task 3: Add error variant for invalid compatibility (AC: #2, #3)
  - [x] Add `InvalidCompatibilityLevel(String)` variant to `KoraError` in `src/error.rs`
  - [x] Confluent error code: 42203
  - [x] HTTP status: 422 (Unprocessable Entity)

- [x] Task 4: Wire routes in api/mod.rs (AC: #1–#5)
  - [x] `GET /config` + `PUT /config`
  - [x] `GET /config/{subject}` + `PUT /config/{subject}` + `DELETE /config/{subject}`
  - [x] Declare `pub mod compatibility;` in `src/api/mod.rs`
  - [x] Declare `pub mod compatibility;` in `src/storage/mod.rs`

- [x] Task 5: Integration tests in tests/api_compatibility_config.rs (AC: #1–#5)
  - [x] Test helpers in `tests/common/api.rs`: `get_global_compatibility`, `set_global_compatibility`, `get_subject_compatibility`, `set_subject_compatibility`, `delete_subject_compatibility`
  - [x] `get_global_compatibility_returns_backward_default` (AC #1)
  - [x] `set_global_compatibility_updates_level` (AC #2) — verify via subsequent GET
  - [x] `get_subject_compatibility_falls_back_to_global` (AC #4 fallback)
  - [x] `set_subject_compatibility_sets_override` (AC #3) — verify via subsequent GET
  - [x] `delete_subject_compatibility_falls_back_to_global` (AC #5) — set, delete, verify fallback
  - [x] `subject_compatibility_returns_404_for_unknown_subject` (GET, PUT, DELETE — 404 + error_code 40401)
  - [x] `set_global_compatibility_rejects_invalid_level` (422 + error_code 42203)
  - [x] `set_subject_compatibility_rejects_invalid_level` (422 + error_code 42203)
  - [x] `set_global_compatibility_accepts_all_valid_levels` (all 7 levels via `COMPATIBILITY_LEVELS`)
  - [x] `get_subject_compatibility_returns_override_not_global` (global=FULL, subject=NONE → returns NONE)

- [x] Task 6: Verify all tests pass
  - [x] `cargo clippy` — zero warnings (pedantic)
  - [x] `cargo test` — 113 tests pass

- [x] Task 7: Naming standardization refactor
  - [x] `serial_test` crate added for test isolation on shared global config
  - [x] All storage module functions renamed to be fully explicit (no implicit context)
  - [x] All API handler functions renamed to be fully explicit
  - [x] `COMPATIBILITY_LEVELS` constant made public and imported in tests (no duplication)
  - [x] `hard_delete_version` test helper moved to correct section (Delete operations)
  - [x] `find_schema_by_subject_name_and_fingerprint` eliminated — replaced by `find_schema_by_subject_id_and_fingerprint` + `find_subject_id_by_name`

### Review Findings

- [x] [Review][Patch] UNION ALL has no guaranteed ordering in `get_level()` — fixed: replaced with `ORDER BY subject IS NULL LIMIT 1`
- [x] [Review][Patch] Missing test: subject override wins when global is non-default — added `get_subject_compatibility_returns_override_not_global`
- [x] [Review][Patch] Dead code `get_subject_level()` — removed
- [x] [Review][Patch] Tests racing on shared global config — fixed with `serial_test` crate + `#[serial]`
- [x] [Review][Patch] 404 tests missing `error_code` assertion — added `assert_eq!(body["error_code"], 40401)` for all 3 endpoints
- [x] [Review][Patch] `COMPATIBILITY_LEVELS` duplicated in tests — made constant public + imported
- [x] [Review][Defer] `delete_subject_level()` not atomic (DELETE + SELECT) — pre-existing pattern
- [x] [Review][Defer] Missing global config row would cause 500 — migration seeds it, pre-existing
- [x] [Review][Defer] TOCTOU race on subject existence check — same pattern as all handlers

### Confluent API Compatibility Verification

| Check | Status |
|-------|--------|
| Endpoints: GET/PUT /config, GET/PUT/DELETE /config/{subject} | PASS |
| GET response: `{"compatibilityLevel": "..."}` | PASS |
| PUT request: `{"compatibility": "..."}` | PASS |
| PUT response: `{"compatibility": "..."}` | PASS |
| DELETE response: `{"compatibility": "..."}` (fallback) | PASS |
| All 7 levels: BACKWARD, BACKWARD_TRANSITIVE, FORWARD, FORWARD_TRANSITIVE, FULL, FULL_TRANSITIVE, NONE | PASS |
| Error 42203 (invalid level) + HTTP 422 | PASS |
| Error 40401 (subject not found) + HTTP 404 | PASS |
| Default level: BACKWARD | PASS |
| Content-Type: application/vnd.schemaregistry.v1+json | PASS |
| Per-subject GET fallback to global | PASS |

## Dev Notes

### Database Schema (Already Exists)

The `config` table is already created in `migrations/001_initial_schema.sql`:
```sql
CREATE TABLE IF NOT EXISTS config (
    id                  BIGSERIAL PRIMARY KEY,
    subject             TEXT UNIQUE,          -- NULL = global config
    compatibility_level TEXT NOT NULL DEFAULT 'BACKWARD',
    mode                TEXT NOT NULL DEFAULT 'READWRITE',
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Pre-seeded global row:
INSERT INTO config (subject, compatibility_level, mode)
VALUES (NULL, 'BACKWARD', 'READWRITE') ON CONFLICT DO NOTHING;
```

No migration needed — the table and seed data already exist.

### Response Shape Asymmetry (Confluent Compatibility)

Confluent API uses different field names for GET vs PUT:
- **GET** responses use `"compatibilityLevel"` key
- **PUT** request and response use `"compatibility"` key
- **DELETE** response uses `"compatibility"` key (returning the fallback level)

This is intentional Confluent behavior — do NOT normalize the field names.

### Subject Existence Check

Per-subject endpoints (`GET /config/{subject}`, `PUT /config/{subject}`, `DELETE /config/{subject}`) validate the subject exists in the `subjects` table (active, not deleted) before proceeding, using `subjects::subject_exists()`.

### Test Isolation Strategy

- Tests that mutate the global config row are marked `#[serial]` (via `serial_test` crate)
- Per-subject tests use UUID-based subject names to avoid collisions
- Global-mutating tests restore `BACKWARD` default after each test

### Architecture Compliance

- One storage module per concern: `src/storage/compatibility.rs`
- One API module per concern: `src/api/compatibility.rs`
- sqlx raw queries (no ORM), `query_scalar` for single-value returns
- Confluent-compatible error codes and response shapes
- `#![deny(missing_docs)]` — all public items need doc comments
- `#![deny(clippy::pedantic)]` — zero warnings
- All function names fully explicit across all modules (no implicit context)

### References

- [Source: epics.md — Epic 4, Story 4.1]
- [Source: prd.md — FR21, FR22, FR23, FR24, FR25]
- [Source: architecture.md — config table schema, API patterns]
- [Source: 3-2-protobuf-format-handler.md — patterns and approach]
- [Source: migrations/001_initial_schema.sql — config table definition]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (1M context)

### Completion Notes List

- Created `src/storage/compatibility.rs` with 5 query functions (get_global_level, set_global_level, get_level with fallback, set_subject_level upsert, delete_subject_level)
- Created `src/api/compatibility.rs` with 5 handlers + level validation against `COMPATIBILITY_LEVELS` constant
- Added `InvalidCompatibilityLevel` error variant (42203, 422) to `src/error.rs`
- Wired 5 routes in `src/api/mod.rs`: GET/PUT /config, GET/PUT/DELETE /config/{subject}
- Added 5 test helpers in `tests/common/api.rs`
- Created `tests/api_compatibility_config.rs` with 10 integration tests covering all 5 ACs + edge cases
- Added `serial_test` dev-dependency for test isolation on shared global config
- Standardized all function names across storage and API modules for full explicitness
- Eliminated `find_schema_by_subject_name_and_fingerprint` duplication — added `find_subject_id_by_name` + `find_schema_by_subject_id_and_fingerprint`
- 113 tests passing, zero clippy warnings

### File List

**New:**
- `src/storage/compatibility.rs`
- `src/api/compatibility.rs`
- `tests/api_compatibility_config.rs`

**Modified:**
- `Cargo.toml` — added `serial_test` dev-dependency
- `src/storage/mod.rs` — added `pub mod compatibility`
- `src/storage/subjects.rs` — renamed functions + added `find_subject_id_by_name`
- `src/storage/schemas.rs` — renamed functions + replaced `find_schema_by_subject_name_and_fingerprint` with `find_schema_by_subject_id_and_fingerprint`
- `src/storage/references.rs` — renamed `insert` → `insert_references`
- `src/api/mod.rs` — added `pub mod compatibility`, wired 5 routes, updated all handler references
- `src/api/subjects.rs` — renamed functions, updated all storage call sites, refactored `check_schema` to use `find_subject_id_by_name`
- `src/api/schemas.rs` — renamed `list_types` → `list_schema_types`, updated storage call sites
- `src/api/health.rs` — renamed `health` → `check_health`
- `src/error.rs` — added `InvalidCompatibilityLevel` variant (42203)
- `tests/common/api.rs` — added 5 compatibility helpers, moved `hard_delete_version` to correct section

### Change Log

- 2026-04-09: Story 4.1 implemented — Compatibility configuration CRUD (FR21–FR25)
- 2026-04-10: Code review — fixed UNION ALL ordering, added serial_test, added override priority test, added error_code assertions
- 2026-04-10: Naming standardization — all storage + API functions renamed for full explicitness across all modules
- 2026-04-10: Refactored duplicate fingerprint queries — eliminated `find_schema_by_subject_name_and_fingerprint`, added `find_subject_id_by_name`
- 2026-04-10: Confluent API compatibility verified — all 11 checks pass
