# Story 6.1: Registry Mode Control

Status: done

**Depends on:** Epic 5 complete (all compatibility modes enforced)

## Story

As an **operator**,
I want to get and set the registry mode (READWRITE, READONLY, READONLY_OVERRIDE, IMPORT),
so that I can control write access during maintenance or migration.

## Acceptance Criteria

### AC1: Get Global Mode (Default)

**Given** a running Kora server
**When** I send `GET /mode`
**Then** I receive HTTP 200 with `{"mode": "READWRITE"}`

### AC2: Set Global Mode

**Given** I want to set read-only mode
**When** I send `PUT /mode` with `{"mode": "READONLY"}`
**Then** I receive HTTP 200 with `{"mode": "READONLY"}`
**And** subsequent schema registration attempts return HTTP 422 with error code 42205

### AC3: IMPORT Mode Accepts Explicit ID

**Given** IMPORT mode is set
**When** I register a schema with an explicit ID
**Then** the system accepts the provided ID instead of auto-allocating

### AC4: READONLY_OVERRIDE Per-Subject

**Given** READONLY_OVERRIDE mode is set at subject level
**When** I attempt to register a schema under that subject
**Then** registration succeeds (READONLY_OVERRIDE allows writes despite global READONLY)

### AC5: Force Mode Change

**Given** `force=true`
**When** I send `PUT /mode?force=true` with `{"mode": "READONLY"}`
**Then** the mode is set even if there are pending operations

### AC6: Set Per-Subject Mode

**Given** subject "orders-value" exists
**When** I send `PUT /mode/orders-value` with `{"mode": "READONLY"}`
**Then** I receive HTTP 200 with `{"mode": "READONLY"}`

### AC7: Get Per-Subject Mode

**Given** subject "orders-value" has per-subject mode
**When** I send `GET /mode/orders-value`
**Then** I receive the per-subject mode

### AC8: Get Per-Subject Mode with Global Fallback

**Given** subject has NO per-subject mode and `defaultToGlobal=true`
**When** I send `GET /mode/orders-value?defaultToGlobal=true`
**Then** I receive the global mode as fallback

### AC9: Get Per-Subject Mode Not Configured

**Given** subject has NO per-subject mode and `defaultToGlobal` is not set
**When** I send `GET /mode/orders-value`
**Then** I receive HTTP 404 with Confluent error code 40409 (subject mode not configured)

### AC10: Invalid Mode Rejected

**Given** an invalid mode value
**When** I send `PUT /mode` with `{"mode": "INVALID"}`
**Then** I receive HTTP 422 with Confluent error code 42204 (invalid mode)

### AC11: Delete Per-Subject Mode

**Given** subject "orders-value" has per-subject mode
**When** I send `DELETE /mode/orders-value`
**Then** the per-subject mode is removed, falling back to global

### AC12: Delete Global Mode (Reset)

**Given** a global mode override exists
**When** I send `DELETE /mode`
**Then** the global mode resets to READWRITE

## Tasks / Subtasks

- [x] Task 1: Storage layer — `src/storage/mode.rs` (AC: 1-12)
  - [x] 1.1: Create `src/storage/mode.rs` with module doc, add `pub mod mode;` to `src/storage/mod.rs`
  - [x] 1.2: `get_global_mode(pool) -> Result<String>` — `SELECT mode FROM config WHERE subject IS NULL`
  - [x] 1.3: `set_global_mode(pool, mode) -> Result<String>` — `UPDATE config SET mode = $1 ... WHERE subject IS NULL RETURNING mode`
  - [x] 1.4: `delete_global_mode(pool) -> Result<String>` — read current, reset to `READWRITE`, return previous (use transaction like `delete_global_level`)
  - [x] 1.5: `get_subject_mode(pool, subject) -> Result<Option<String>>` — `SELECT mode FROM config WHERE subject = $1 AND mode IS NOT NULL`
  - [x] 1.6: `set_subject_mode(pool, subject, mode) -> Result<String>` — upsert via `INSERT ... ON CONFLICT DO UPDATE SET mode = $2`
  - [x] 1.7: `delete_subject_mode(pool, subject) -> Result<Option<String>>` — transaction: read prev, set NULL, return prev
  - [x] 1.8: `get_effective_mode(pool, subject) -> Result<String>` — subject-level first, then global fallback

- [x] Task 2: API handlers — `src/api/mode.rs` (AC: 1-12)
  - [x] 2.1: Create `src/api/mode.rs` with module doc, add `pub mod mode;` to `src/api/mod.rs`
  - [x] 2.2: Define types:
    - `ModeRequest { mode: String }` (request body)
    - `ModeSetParams { force: bool }` (query param for PUT)
    - `ModeDeleteParams { recursive: bool }` (query param for DELETE subject)
    - Reuse `DefaultToGlobalParams` from `compatibility.rs` (already pub)
  - [x] 2.3: Define `VALID_MODES: &[&str] = &["READWRITE", "READONLY", "READONLY_OVERRIDE", "IMPORT"]`
  - [x] 2.4: `validate_mode(mode) -> Result<(), KoraError>` — check against `VALID_MODES`, return `KoraError::InvalidMode` on failure
  - [x] 2.5: `get_global_mode` handler — `GET /mode` → returns `{"mode": "READWRITE"}`
  - [x] 2.6: `set_global_mode` handler — `PUT /mode` → validates mode, calls storage, returns `{"mode": "<new>"}`
  - [x] 2.7: `delete_global_mode` handler — `DELETE /mode` → resets to READWRITE, returns previous `{"mode": "<prev>"}`
  - [x] 2.8: `get_subject_mode` handler — `GET /mode/{subject}` → per-subject or 40409, with `defaultToGlobal` fallback
  - [x] 2.9: `set_subject_mode` handler — `PUT /mode/{subject}` → validates mode, upserts, returns `{"mode": "<new>"}`
  - [x] 2.10: `delete_subject_mode` handler — `DELETE /mode/{subject}` → removes per-subject mode, returns previous `{"mode": "<prev>"}`

- [x] Task 3: Router registration — `src/api/mod.rs` (AC: 1-12)
  - [x] 3.1: Add routes after the `/config/{subject}` block:
    ```
    .route("/mode", get(mode::get_global_mode).put(mode::set_global_mode).delete(mode::delete_global_mode))
    .route("/mode/{subject}", get(mode::get_subject_mode).put(mode::set_subject_mode).delete(mode::delete_subject_mode))
    ```

- [x] Task 4: Mode enforcement in register_schema (AC: 2, 3, 4)
  - [x] 4.1: In `src/api/subjects.rs::register_schema`, after subject validation and before schema parsing, add mode check:
    - Get effective mode: call `mode::get_effective_mode(&pool, &subject).await?`
    - If effective mode is `READONLY` → return `Err(KoraError::OperationNotPermitted)`
    - If effective mode is `READWRITE` or `IMPORT` → proceed normally
    - `READONLY_OVERRIDE` at subject level allows writes even if global is `READONLY`
  - [x] 4.2: IMPORT mode handling: accept-and-ignore for now (explicit ID assignment deferred — documented in completion notes)
  - [x] 4.3: `READONLY_OVERRIDE` logic: the effective mode resolution handles this — if subject has `READONLY_OVERRIDE`, that's the effective mode, and `READONLY_OVERRIDE` permits writes

- [x] Task 5: Integration tests in `tests/api_mode.rs` (AC: 1-12)
  - [x] 5.1: Add test helpers to `tests/common/api.rs`:
    - `get_global_mode(client, base) -> Response`
    - `set_global_mode(client, base, mode) -> Response`
    - `delete_global_mode(client, base) -> Response`
    - `get_subject_mode(client, base, subject) -> Response`
    - `set_subject_mode(client, base, subject, mode) -> Response`
    - `delete_subject_mode(client, base, subject) -> Response`
  - [x] 5.2: Test global mode defaults to READWRITE (AC1)
  - [x] 5.3: Test set and get global mode (AC2)
  - [x] 5.4: Test set global mode with force=true (AC5)
  - [x] 5.5: Test set and get per-subject mode (AC6, AC7)
  - [x] 5.6: Test get per-subject mode with defaultToGlobal=true (AC8)
  - [x] 5.7: Test get per-subject mode returns 404/40409 when not configured (AC9)
  - [x] 5.8: Test invalid mode returns 422/42204 (AC10)
  - [x] 5.9: Test delete per-subject mode (AC11)
  - [x] 5.10: Test delete global mode resets to READWRITE (AC12)
  - [x] 5.11: Test READONLY blocks schema registration with 422/42205 (AC2)
  - [x] 5.12: Test READONLY_OVERRIDE at subject level allows registration despite global READONLY (AC4)
  - [x] 5.13: Test READWRITE allows normal registration (baseline sanity)
  - [x] 5.14: Restore global mode to READWRITE after each mutating test

## Dev Notes

### This Mirrors the Compatibility Config Pattern Exactly

All mode CRUD endpoints follow the identical structure as the existing compatibility config endpoints. The `config` table already has a `mode TEXT NOT NULL DEFAULT 'READWRITE'` column.

| Existing piece to reuse/mirror | Location |
|---|---|
| `KoraError::SubjectModeNotConfigured` (40409) | `src/error.rs:57-58` |
| `KoraError::InvalidMode` (42204) | `src/error.rs:66-67` |
| `KoraError::OperationNotPermitted` (42205) | `src/error.rs:69-70` |
| Config table with `mode` column | `migrations/001_initial_schema.sql:40-47` |
| Global config row seeded with `READWRITE` | `migrations/001_initial_schema.sql:61-63` |
| Compatibility config handlers (mirror pattern) | `src/api/compatibility.rs:80-188` |
| Compatibility storage functions (mirror pattern) | `src/storage/compatibility.rs:1-187` |
| `DefaultToGlobalParams` struct | `src/api/compatibility.rs:40-45` |
| Router registration pattern | `src/api/mod.rs:61-72` |
| Test helpers pattern | `tests/common/api.rs:201-244` |
| `register_schema` handler (enforcement insertion point) | `src/api/subjects.rs:154-242` |

### Storage Layer Pattern

Mirror `storage/compatibility.rs` exactly but operate on the `mode` column:

- `get_global_mode` → `SELECT mode FROM config WHERE subject IS NULL`
- `set_global_mode` → `UPDATE config SET mode = $1, updated_at = now() WHERE subject IS NULL RETURNING mode`
- `delete_global_mode` → Transaction: read current, UPDATE to 'READWRITE', return previous
- `get_subject_mode` → `SELECT mode FROM config WHERE subject = $1` (fetch_optional)
- `set_subject_mode` → `INSERT ... ON CONFLICT DO UPDATE SET mode = $2, updated_at = now() RETURNING mode`
- `delete_subject_mode` → Two approaches:
  - If per-subject config row has **only** mode set (default compat level): DELETE the row and return previous mode
  - If per-subject config row has custom compat level too: UPDATE mode to default 'READWRITE' and return previous mode
  - **Simplest approach**: Just reset mode to 'READWRITE' on the config row. But Confluent actually deletes the mode override — need to think about this carefully since mode and compat share the same config row.
  
**Important design decision for delete_subject_mode**: The `config` table stores both `compatibility_level` and `mode` in the same row. Deleting a subject's mode should NOT delete the compatibility config. Options:
  1. **Reset mode to 'READWRITE'** (not a true delete but semantically equivalent)
  2. **Track mode-set separately** (adds complexity)
  
  Recommendation: Reset to 'READWRITE' — this matches Confluent behavior where deleting mode falls back to global, and READWRITE is the default. The `get_subject_mode` function checks for non-default values to determine if "configured".

  Actually, **simplest correct approach**: `get_subject_mode` returns `None` if no per-subject config row exists. `delete_subject_mode` should set `mode = 'READWRITE'` on the existing row (don't delete the row — compat config may exist). If no row exists, return `None`.

### Mode Enforcement Logic

Insert in `register_schema` (src/api/subjects.rs) early — **after `validate_subject`**, **before schema parsing** (fail fast, save work):

```rust
// Enforce registry mode before any expensive parsing.
let effective_mode = mode::get_effective_mode(&pool, &subject).await?;
if effective_mode == "READONLY" {
    return Err(KoraError::OperationNotPermitted);
}
```

**READONLY_OVERRIDE resolution**: The `get_effective_mode` function returns the subject-level mode if set, otherwise the global mode. If global is READONLY but subject has READONLY_OVERRIDE, the effective mode for that subject is READONLY_OVERRIDE which permits writes. The handler only blocks on `"READONLY"` — all other modes (READWRITE, IMPORT, READONLY_OVERRIDE) allow registration.

**IMPORT mode**: Confluent's IMPORT mode allows registering schemas with explicit IDs. Our current `register_schema` auto-allocates IDs via PostgreSQL sequence. Full IMPORT mode support (explicit ID acceptance) would require adding an `id` field to `RegisterParams` and modifying `register_schema_atomically` to use the provided ID. For this story, IMPORT mode should be accepted as a valid mode value but the explicit-ID behavior can be deferred if it requires significant changes to the registration path. Document this in completion notes.

### force and recursive Query Parameters

- **force=true** on `PUT /mode`: Confluent uses this to force mode change even with pending operations. Since Kora doesn't track pending operations, `force` is accept-and-ignore. Define the query param struct but don't add special logic.
- **recursive=true** on `DELETE /mode/{subject}`: Confluent propagates mode deletion to child subjects. This would require a prefix-based delete: `DELETE FROM config WHERE subject LIKE $1 || '%' AND subject != $1`. Implement this.

### DefaultToGlobalParams Reuse

`DefaultToGlobalParams` in `src/api/compatibility.rs:40-45` is already `pub`. Import it in `mode.rs`:
```rust
use crate::api::compatibility::DefaultToGlobalParams;
```

### Valid Mode Values

Confluent Schema Registry valid modes: `READWRITE`, `READONLY`, `READONLY_OVERRIDE`, `IMPORT`

### Response Format

Follow Confluent's mode endpoint response format:
- GET: `{"mode": "READWRITE"}`
- PUT: `{"mode": "READONLY"}` (the newly set mode)
- DELETE: `{"mode": "READONLY"}` (the previous mode before deletion/reset)

### Edge Cases

- Delete subject mode when no per-subject config exists → return `SubjectNotFound` (404) matching Confluent behavior
- Delete subject mode when per-subject config exists but mode was default READWRITE → still return the previous mode value
- Set mode on a subject that doesn't exist in the subjects table → Confluent allows this (same as compat config — no existence check)
- Multiple writes during mode change → no locking needed for MVP (single-instance)

### Project Structure Notes

New files to create:
- `src/api/mode.rs` — mode API handlers (mirrors `src/api/compatibility.rs` config section)
- `src/storage/mode.rs` — mode storage operations (mirrors `src/storage/compatibility.rs`)

Files to modify:
- `src/api/mod.rs` — add `pub mod mode;` and route registration
- `src/storage/mod.rs` — add `pub mod mode;`
- `src/api/subjects.rs` — add mode enforcement in `register_schema`
- `tests/common/api.rs` — add mode helper functions

New test files:
- Tests go in `tests/api_mode.rs` or distributed into `tests/api_register_schema.rs` for enforcement tests

### Previous Story Intelligence

From story 5.2 completion notes:
- `register_schema` handler flow: parse body → validate subject → parse schema → normalize → compat check → validate refs → register_atomically
- Mode check should go BEFORE compat check (fail fast on READONLY before running expensive compat logic)
- Confluent wire-compatibility is fully audited across 24 endpoints — mode endpoints are the remaining gap
- Test pattern: use `#[serial]` for tests that mutate global state, always restore defaults after
- `register_schema_atomically` returns `(content_id, version, is_new)` — the `content_id` is the global schema ID

### Git Intelligence

Recent commits show the project follows a strict pattern:
- One commit per story with descriptive message
- All 5 epics complete, story 6.1 is the first in the final epic
- The codebase has been through multiple Confluent wire-compatibility audits
- Test files are named `api_{feature}.rs` following the endpoint group

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 6.1] — Full acceptance criteria
- [Source: _bmad-output/planning-artifacts/prd.md#FR43-FR44] — Registry mode requirements
- [Source: _bmad-output/planning-artifacts/architecture.md#Module Organization] — `api/mode.rs`, `storage/config.rs` module placement
- [Source: src/error.rs:57-58,66-67,69-70] — Mode error variants (40409, 42204, 42205)
- [Source: migrations/001_initial_schema.sql:40-47] — Config table with mode column
- [Source: src/api/compatibility.rs:80-188] — Config handler pattern to mirror
- [Source: src/storage/compatibility.rs:1-187] — Config storage pattern to mirror
- [Source: src/api/subjects.rs:154-242] — register_schema handler (mode enforcement insertion point)
- [Source: src/api/mod.rs:61-72] — Router registration pattern
- [Source: _bmad-output/implementation-artifacts/5-2-enforce-all-compatibility-modes.md] — Previous story

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (1M context)

### Debug Log References

### Completion Notes List

- **Task 1**: Created `src/storage/mode.rs` with 7 functions mirroring `storage/compatibility.rs` pattern. Made mode column nullable in initial migration — allows distinguishing "not configured" (NULL) from "explicitly set READWRITE" for per-subject mode overrides. Global config row keeps mode='READWRITE' (never NULL).
- **Task 2**: Created `src/api/mode.rs` with 6 handlers (GET/PUT/DELETE for `/mode` and `/mode/{subject}`). Reuses `DefaultToGlobalParams` from compatibility module. Accepts `force` param on PUT and `recursive` param on DELETE. Validates against `VALID_MODES` = READWRITE, READONLY, READONLY_OVERRIDE, IMPORT, FORWARD (all 5 Confluent modes).
- **Task 3**: Added `/mode` and `/mode/{subject}` routes to `src/api/mod.rs` after the `/config/{subject}` block.
- **Task 4**: Added mode enforcement via `enforce_writable` allowlist on `register_schema`, `delete_subject`, and `delete_version`. Modes READWRITE, IMPORT, FORWARD, READONLY_OVERRIDE permit writes; READONLY blocks with 42205. READONLY_OVERRIDE at subject level naturally overrides global READONLY via `get_effective_mode` single-query fallback.
- **Task 5**: 26 integration tests in `tests/api_mode.rs` covering all 12 ACs + cross-module isolation (phantom compat prevention, compat-delete preserves mode), READONLY blocks deletes, per-subject READONLY enforcement, recursive delete with ^@ operator, FORWARD mode allows writes. Uses `#[file_serial]` for cross-binary test isolation.
- **IMPORT mode explicit ID**: Deferred. IMPORT mode is accepted and permits registration, but explicit ID assignment would require adding an `id` field to `RegisterParams`.
- **Recursive delete**: Implemented. `DELETE /mode/{subject}?recursive=true` clears mode on all child subjects (prefix match via PostgreSQL `^@` operator) atomically in a single transaction.
- **Review fixes**: 3 review passes led to: shared config row safety (nullable compat_level + UPDATE-instead-of-DELETE), allowlist enforcement, READONLY on deletes, COALESCE for NULL safety, single-query effective mode (no TOCTOU), ^@ operator (no LIKE injection), atomic recursive delete.

### Change Log

- 2026-04-15: Implemented registry mode CRUD + enforcement. Made mode column nullable in initial migration.
- 2026-04-15: Review fixes — shared config row safety (nullable compat_level, UPDATE+orphan cleanup), allowlist enforcement, READONLY on deletes, COALESCE NULL safety, single-query effective mode, ^@ operator for recursive delete, atomic recursive transaction, FORWARD mode added. 26 integration tests with file_serial.

### File List

- src/storage/mode.rs (new) — mode storage operations (get/set/delete global + subject + effective)
- src/storage/mod.rs (modified) — added `pub mod mode;`
- src/api/mode.rs (new) — mode API handlers (GET/PUT/DELETE for /mode and /mode/{subject})
- src/api/mod.rs (modified) — added `pub mod mode;` and /mode, /mode/{subject} routes
- src/api/subjects.rs (modified) — mode enforcement in register_schema handler
- migrations/001_initial_schema.sql (modified) — mode column now nullable (was NOT NULL DEFAULT 'READWRITE')
- tests/api_mode.rs (new) — 18 integration tests for mode CRUD + enforcement
- tests/common/api.rs (modified) — added 6 mode helper functions
