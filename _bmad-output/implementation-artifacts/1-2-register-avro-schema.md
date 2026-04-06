# Story 1.2: Register Avro Schema

Status: done

## Story

As a **developer**,
I want to register an Avro schema under a subject,
so that my producers and consumers can serialize/deserialize data using a shared schema.

## Acceptance Criteria

1. Valid Avro schema → `POST /subjects/{subject}/versions` with `{"schema": "<avro_json>"}` → HTTP 200 with `{"id": <globally_unique_sequential_id>}`, schema stored with canonical form
2. Same Avro schema registered again under same subject → returns existing schema ID (idempotent, no duplicate version)
3. Invalid Avro schema → HTTP 422 with Confluent error code 42201 (`{"error_code": 42201, "message": "Invalid schema: ..."}`)
4. `schemaType` omitted → defaults to AVRO (Confluent default behavior)

## Tasks / Subtasks

- [x] Task 1: Avro schema parsing module (AC: #1, #3, #4)
  - [x] Create `src/schema/mod.rs` with `SchemaFormat` enum (Avro only for now) and `SchemaHandler` trait
  - [x] Create `src/schema/avro.rs` with `AvroHandler` — `parse()`, `canonical_form()`, `fingerprint()`
  - [x] Use `apache_avro::Schema::parse_str()` for validation — failure = `KoraError::InvalidSchema`
  - [x] Export `schema` module from `src/lib.rs`
  - [x] TDD: test valid schema parses, test invalid schema errors, test canonical form stable, test fingerprint stable

- [x] Task 2: Storage layer for subjects and schemas (AC: #1, #2)
  - [x] Create `src/storage/subjects.rs` — `upsert_subject(pool, name) -> Result<i64>` (INSERT ... ON CONFLICT DO NOTHING + SELECT id)
  - [x] Create `src/storage/schemas.rs`:
    - `find_by_fingerprint(pool, subject_id, fingerprint) -> Result<Option<i64>>` for idempotency check
    - `next_version(pool, subject_id) -> Result<i32>` (MAX(version) + 1, default 1)
    - `insert_schema(pool, subject_id, version, schema_type, schema_text, canonical_form, fingerprint) -> Result<i64>` returning schema ID
  - [x] Re-export sub-modules from `src/storage/mod.rs`
  - [x] TDD: test subject upsert idempotent, test schema insert returns ID, test find_by_fingerprint returns None/Some

- [x] Task 3: API handler for POST /subjects/{subject}/versions (AC: #1, #2, #3, #4)
  - [x] Create `src/api/subjects.rs` with `register_schema` handler
  - [x] Request body: `RegisterSchemaRequest { schema: String, schema_type: Option<String> }` (serde: `schemaType`)
  - [x] Flow: parse Avro → compute canonical + fingerprint → upsert subject → check fingerprint → insert or return existing ID
  - [x] Response: `{"id": <i64>}` with HTTP 200
  - [x] Register route in `src/api/mod.rs`: `.route("/subjects/{subject}/versions", post(subjects::register_schema))`
  - [x] TDD: integration test valid registration, duplicate returns same ID, invalid schema 422

- [x] Task 4: Verify CI-readiness (AC: #1-#4)
  - [x] `make lint` — zero warnings
  - [x] `make test` — all tests pass
  - [x] Verify idempotency with integration test

## Dev Notes

### Confluent API Contract

```
POST /subjects/{subject}/versions
Content-Type: application/vnd.schemaregistry.v1+json

Request:  {"schema": "<json_string>", "schemaType": "AVRO"}  ← schemaType optional
Response: {"id": 1}                                          ← globally unique sequential ID
```

- Subjects are created implicitly on first schema registration (no separate "create subject" endpoint)
- `schemaType` defaults to `"AVRO"` when absent
- Schema ID comes from `schemas.id` (BIGSERIAL PK) — PostgreSQL handles sequential allocation
- Version auto-increments per subject (1, 2, 3...) — not user-specified

### Idempotency Logic

```
1. Parse schema → compute canonical_form + fingerprint
2. Upsert subject (INSERT ON CONFLICT DO NOTHING)
3. SELECT id FROM schemas WHERE subject_id = ? AND fingerprint = ? AND deleted = false
4. If found → return existing id (HTTP 200)
5. If not → determine next version, INSERT, return new id (HTTP 200)
```

### Apache Avro Crate Usage

```rust
use apache_avro::Schema;

// Parse + validate
let schema = Schema::parse_str(&raw_json)?;  // Err = invalid

// Canonical form (for dedup)
let canonical = schema.canonical_form();

// Fingerprint (for fast lookup)
use apache_avro::rabin::rabin_fingerprint64;
let fp = rabin_fingerprint64(canonical.as_bytes());
```

### Architecture Compliance

- **Handler pattern**: `async fn handler(State(pool), Path(subject), Json(body)) -> Result<Response, KoraError>` — match health.rs pattern
- **Error pattern**: Return `KoraError::InvalidSchema(msg)` on parse failure — already maps to 422/42201
- **Storage pattern**: Standalone async functions taking `&PgPool` as first arg
- **No `unwrap()`** outside `#[cfg(test)]`
- **`deny(missing_docs)` + `deny(clippy::pedantic)`** enforced

### File Structure

```
NEW:
├── src/schema/mod.rs           ← SchemaFormat enum, SchemaHandler trait
├── src/schema/avro.rs          ← AvroHandler impl
├── src/api/subjects.rs         ← POST /subjects/{subject}/versions handler
├── src/storage/subjects.rs     ← Subject upsert
├── src/storage/schemas.rs      ← Schema insert, fingerprint lookup, version calc
├── tests/register_schema.rs    ← Integration tests

MODIFIED:
├── src/lib.rs                  ← add `pub mod schema;`
├── src/api/mod.rs              ← add route + mod subjects
├── src/storage/mod.rs          ← add mod subjects, schemas + re-exports
```

### Previous Story Intelligence

From story 1.1 completion notes:
- Content-Type handled globally by middleware (don't set per-handler)
- `schemas.version` has `CHECK (version > 0)` — next_version must start at 1
- Integration tests split by domain in `tests/`, shared helpers in `tests/common/mod.rs`
- `DATABASE_URL` required via env (no hardcoded fallback)
- 15 tests currently (12 unit + 3 integration)

### Testing Strategy

- **Unit tests** (co-located `#[cfg(test)]`):
  - `schema/avro.rs`: parse valid/invalid, canonical form stability, fingerprint stability
  - `storage/schemas.rs`: (integration-only — needs PG)
- **Integration tests** (`tests/register_schema.rs`):
  - POST valid schema → 200 + `{"id": N}`
  - POST same schema again → 200 + same ID (idempotency)
  - POST invalid schema → 422 + error code 42201
  - POST without schemaType → defaults to AVRO, 200
  - POST to new subject → subject auto-created

### References

- [Source: epics.md — Epic 1, Story 1.2]
- [Source: prd.md — FR1, FR15, FR49]
- [Source: architecture.md — Write Path, Schema Handling, Storage Patterns]
- [Source: architecture.md — Code Philosophy, Implementation Patterns]
- [Source: 1-1-project-scaffold-database-health.md — Completion Notes]

### Review Findings

- [x] [Review][Fixed] TOCTOU race: merged `next_version` + `insert` into atomic `INSERT ... SELECT COALESCE(MAX(version),0)+1`. No separate read-then-write gap.
- [x] [Review][Fixed] Case-sensitive `schemaType` matching — now uses `str::to_ascii_uppercase` before matching, accepts `"avro"`, `"Avro"`, etc.
- [x] [Review][Fixed] Subject name validation — rejects empty, NUL bytes, and names > 255 chars.
- [x] [Review][Fixed] Makefile `migrate` glob ordering — now pipes through `sort` for deterministic order.
- [x] [Review][Fixed] `From<sqlx::Error>` preserves error context — `BackendDataStore(String)` now carries the inner message.
- [x] [Review][Fixed] Soft-delete version numbering — atomic INSERT uses `MAX(version)` across all rows (not just `deleted=false`), so versions always monotonically increase.
- [x] [Review][Fixed] Body size limit — `DefaultBodyLimit::max(1MB)` added to router.
- [x] [Review][Fixed] JSON rejection format — handler catches `JsonRejection` and returns Confluent-compatible `{"error_code": 42201, "message": "..."}`.

## Dev Agent Record

### Agent Model Used

### Debug Log References

### Completion Notes List

- 28 tests total: 20 unit + 8 integration (3 from 1.1 + 5 new)
- `SchemaFormat` enum dispatch (not trait) — simple, extensible for JSON Schema / Protobuf later
- Rabin fingerprint via `apache_avro::rabin::Rabin` — stable, hex-encoded string stored in DB
- Storage functions take `&PgPool` directly (no wrapper struct) — simple and testable
- Idempotency: fingerprint-based dedup per subject, returns existing schema ID
- Subjects auto-created on first registration (INSERT ON CONFLICT DO NOTHING)

### File List

- `src/schema/mod.rs` — NEW: SchemaFormat enum, ParsedSchema struct, parse() dispatch
- `src/schema/avro.rs` — NEW: Avro parsing via apache-avro, canonical form, Rabin fingerprint
- `src/api/subjects.rs` — NEW: POST /subjects/{subject}/versions handler
- `src/storage/subjects.rs` — NEW: upsert subject
- `src/storage/schemas.rs` — NEW: find_by_fingerprint, next_version, insert
- `src/lib.rs` — MODIFIED: added `pub mod schema`
- `src/api/mod.rs` — MODIFIED: added subjects route + mod
- `src/storage/mod.rs` — MODIFIED: added sub-module exports
- `tests/register_schema.rs` — NEW: 5 integration tests
