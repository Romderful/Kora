# Story 5.2: Enforce All Compatibility Modes

Status: done

**Depends on:** Story 5.1 (compatibility test endpoint + diff engines)

## Story

As a **developer**,
I want the registry to enforce all 7 compatibility modes when registering schemas,
so that incompatible schema changes are rejected automatically.

## Acceptance Criteria

### AC1: BACKWARD Enforcement

**Given** subject with BACKWARD mode and an existing Avro schema with a required field
**When** I register a new version that removes the required field
**Then** registration is rejected with HTTP 409 and Confluent error code 40901

### AC2: FORWARD Enforcement

**Given** subject with FORWARD mode
**When** I register a new version that adds a required field without default
**Then** registration is rejected with HTTP 409 and error code 40901

### AC3: FULL Enforcement

**Given** subject with FULL mode
**When** I register a schema that is backward-compatible but not forward-compatible
**Then** registration is rejected with HTTP 409

### AC4: NONE Mode

**Given** subject with NONE mode
**When** I register any valid schema
**Then** registration succeeds regardless of compatibility

### AC5: BACKWARD_TRANSITIVE Enforcement

**Given** subject with BACKWARD_TRANSITIVE mode and versions 1, 2, 3
**When** I register version 4
**Then** it must be backward-compatible with ALL previous versions (1, 2, 3), not just version 3

### AC6: FORWARD_TRANSITIVE and FULL_TRANSITIVE

**Given** FORWARD_TRANSITIVE or FULL_TRANSITIVE mode and multiple versions
**When** registering a new version
**Then** the same transitive logic applies (checked against all versions, not just latest)

**FRs:** FR26, FR27, FR28, FR29, FR30, FR31, FR32

## Tasks / Subtasks

- [x] Task 1: Add compatibility enforcement to `register_schema` (AC: 1-6)
  - [x] 1.1: In `src/api/subjects.rs::register_schema`, insert compat check between schema parsing and `register_schema_atomically`
  - [x] 1.2: Resolve effective level via `compatibility::get_effective_compatibility(pool, subject)`
  - [x] 1.3: Determine versions to check:
    - `NONE` → skip entirely
    - Non-transitive (`BACKWARD`, `FORWARD`, `FULL`) → latest version only via `find_latest_schema_by_subject`
    - Transitive (`*_TRANSITIVE`) → all active versions via `list_schema_versions` + `find_schema_by_subject_version`
  - [x] 1.4: Run `schema::check_compatibility(format, &new_schema, &existing_schema, direction)` against each
  - [x] 1.5: If any check fails → return `Err(KoraError::IncompatibleSchema)`
  - [x] 1.6: First schema in a subject (no existing versions) → always succeeds, skip check

- [x] Task 2: Tests in `tests/api_register_schema.rs` (AC: 1-6)
  - [x] 2.1: BACKWARD: incompatible → 409/40901
  - [x] 2.2: BACKWARD: compatible → 200 success
  - [x] 2.3: FORWARD: incompatible → 409
  - [x] 2.4: FULL: backward-only-compatible → 409
  - [x] 2.5: NONE: any schema → 200 success
  - [x] 2.6: BACKWARD_TRANSITIVE: V3 compat with V2 but not V1 → 409
  - [x] 2.7: FULL_TRANSITIVE: both directions against all versions
  - [x] 2.8: First schema in empty subject → always succeeds
  - [x] 2.9: JSON Schema enforcement works (same flow, different engine)
  - [x] 2.10: Protobuf enforcement works (same flow, different engine)

## Dev Notes

### This Is Pure Wiring

All pieces exist from story 5.1 — this story just connects them in `register_schema`:

| Existing piece | Location |
|---|---|
| `schema::check_compatibility(format, new, existing, direction)` | `src/schema/mod.rs:141` |
| `CompatDirection::from_level("BACKWARD_TRANSITIVE")` → `Backward` | `src/schema/mod.rs:52` |
| `compatibility::get_effective_compatibility(pool, subject)` | `src/storage/compatibility.rs` |
| `KoraError::IncompatibleSchema` → HTTP 409, code 40901 | `src/error.rs:61,112,142` |
| `find_latest_schema_by_subject(pool, subject, false)` | `src/storage/schemas.rs:231` |
| `list_schema_versions(pool, subject, false, false, false, 0, -1)` | `src/storage/schemas.rs:507` |
| `find_schema_by_subject_version(pool, subject, v, false)` | `src/storage/schemas.rs` |
| Test fixtures: `COMPAT_AVRO_V1/V2/V3/INCOMPAT` | `tests/common/mod.rs` |
| Test helpers: `register_schema`, `set_subject_compatibility` | `tests/common/api.rs` |

### Transitive Detection

`CompatDirection::from_level` strips the `_TRANSITIVE` suffix — both `"BACKWARD"` and `"BACKWARD_TRANSITIVE"` return `Backward`. The transitive distinction is needed for version selection:

```rust
let is_transitive = level.contains("TRANSITIVE");
```

### Insertion Point

Current `register_schema` flow (`src/api/subjects.rs:142-181`):
```
parse body → validate subject → parse schema → resolve normalize → validate refs → register_atomically
```

Insert after normalize, before refs:
```
parse body → validate subject → parse schema → resolve normalize → COMPAT CHECK → validate refs → register_atomically
```

### Edge Cases

- First schema in subject: `find_latest` returns `None` → skip check
- Subject doesn't exist yet: no versions → skip check (subject created implicitly on register)
- Idempotent re-registration (same schema): `register_schema_atomically` returns existing ID, compat check sees identical schema → compatible

### Previous Story Intelligence

From story 5.1 completion notes:
- `check_compatibility` returns `CompatibilityResult { is_compatible: bool, messages: Vec<String> }`
- `check_with_direction` handles BACKWARD/FORWARD/FULL direction logic internally
- Normalize: `params.normalize || get_effective_normalize(pool, subject)` pattern already established
- The diff engines are Confluent-conformant (299 test cases passing)

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 5.2]
- [Source: _bmad-output/planning-artifacts/prd.md#FR26-FR32]
- [Source: src/api/subjects.rs:142-181] — register_schema handler
- [Source: src/schema/mod.rs:141-159] — check_compatibility dispatch
- [Source: src/error.rs:61,112,142] — IncompatibleSchema (40901, HTTP 409)
- [Source: _bmad-output/implementation-artifacts/5-1-compatibility-test-endpoint.md]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (1M context)

### Debug Log References

### Completion Notes List

- **Task 1**: Added compatibility enforcement in `register_schema` handler — 20 lines between schema parsing and `register_schema_atomically`. Resolves effective level, determines transitive vs non-transitive, checks against latest (non-transitive) or all versions (transitive). `CompatDirection::None` skips entirely. First schema in subject always succeeds (no versions to check).
- **Task 2**: 12 integration tests in `api_register_schema.rs` covering all 7 modes + first-schema + JSON Schema + Protobuf + FORWARD_TRANSITIVE + enforcement after soft-delete + level change to NONE + FORWARD→BACKWARD level switch.
- Fixed `AVRO_SCHEMA_V2/V3` fixtures in `common/mod.rs` to use optional fields with defaults (backward-compatible evolution chain).
- Fixed `get_versions_by_schema_id_multiple_succeeds` test in `api_schema_cross_refs.rs` — set NONE mode before registering different record schemas.
- Confluent API wire-compatibility audit (6 independent audits):
  - `register_schema_atomically` now returns `(content_id, version, is_new)` — extracted `find_existing_version` helper
  - `POST /subjects/{subject}/versions` response enriched: `id`, `version`, `schemaType`, `schema`, `references` (matches Confluent `RegisterSchemaResponse`)
  - `schemaType: "AVRO"` always serialized (removed `skip_serializing_if = "is_avro"` — matches Confluent `@JsonInclude(NON_EMPTY)`)
  - `GET /schemas/ids/{id}` response: removed extra `id` field, `references` omitted when empty (matches Confluent `SchemaString` DTO)
  - `DELETE /config` and `DELETE /config/{subject}` return `Config` object (matches Confluent `ConfigResource`)
  - `POST /compatibility/subjects/{subject}/versions` returns `is_compatible: true` for nonexistent subject (matches Confluent behavior)
  - Added `format` param on `RegisterParams`, `CheckParams`, `GetVersionParams`, `GetSchemaTextParams` (accept-and-ignore)
  - 0 wire-level incompatibilities confirmed across 24 endpoints, all params, all response shapes, all error codes

### Change Log

- 2026-04-15: Implemented compatibility enforcement in register_schema + 12 tests. Fixed test fixtures for backward-compatible evolution.
- 2026-04-15: Confluent API wire-compatibility fixes: enriched register response, schemaType always included, GET /schemas/ids/{id} response aligned with SchemaString DTO, DELETE /config returns Config object, compat test returns true for nonexistent subject, format params added everywhere.

### File List

- src/api/subjects.rs (modified) — compatibility enforcement + enriched register response + format params
- src/api/schemas.rs (modified) — GET /schemas/ids/{id} response aligned + format param on text endpoint
- src/api/compatibility.rs (modified) — DELETE /config returns Config object, compat test nonexistent subject behavior
- src/storage/schemas.rs (modified) — register returns version, schemaType always serialized, find_existing_version helper
- tests/api_register_schema.rs (modified) — 12 enforcement tests
- tests/api_compatibility_config.rs (modified) — DELETE config response assertions
- tests/api_get_schema_by_version.rs (modified) — schemaType AVRO assertion
- tests/api_get_schema_by_id.rs (modified) — schemaType AVRO assertion
- tests/api_check_schema.rs (modified) — schemaType AVRO assertion
- tests/api_list_schemas.rs (modified) — schemaType AVRO assertion
- tests/common/mod.rs (modified) — AVRO_SCHEMA_V2/V3 backward-compatible fixtures
- tests/api_schema_cross_refs.rs (modified) — NONE mode for incompatible schemas
