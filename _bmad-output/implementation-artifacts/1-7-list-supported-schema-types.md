# Story 1.7: List Supported Schema Types

Status: done

## Story

As a **developer**,
I want to list the schema types the registry supports,
so that I know which formats I can use.

## Acceptance Criteria

1. **Given** a running Kora server
   **When** I send `GET /schemas/types`
   **Then** I receive HTTP 200 with `["AVRO", "JSON", "PROTOBUF"]`

**FRs Covered:** FR12

## Tasks / Subtasks

- [x] Task 1: API handler — GET /schemas/types (AC: #1)
  - [x] Add `list_types` handler in `src/api/schemas.rs` — returns static JSON array `["AVRO", "JSON", "PROTOBUF"]`
  - [x] Register route in `src/api/mod.rs`: `GET /schemas/types`

- [x] Task 2: Integration test (AC: #1)
  - [x] Create `tests/api_list_schema_types.rs`
  - [x] Test: GET /schemas/types → 200 with `["AVRO", "JSON", "PROTOBUF"]`

- [x] Task 3: Verify all tests pass
  - [x] `just lint` — zero warnings
  - [x] `just test` — all tests pass (46 existing + new)

## Dev Notes

### Confluent API Contract

```
GET /schemas/types
Accept: application/vnd.schemaregistry.v1+json

Response (200): ["AVRO", "JSON", "PROTOBUF"]
```

Static response — no database query needed. Returns all types the registry will eventually support, matching Confluent behavior.

### Architecture Compliance

- **Handler placement**: `src/api/schemas.rs` (schemas resource)
- **No storage needed** — static response
- **No new structs** — `Json(["AVRO", "JSON", "PROTOBUF"])` inline

### References

- [Source: epics.md — Epic 1, Story 1.7]
- [Source: prd.md — FR12]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (1M context)

### Completion Notes List

- Static handler, no DB, no struct — `Json(["AVRO", "JSON", "PROTOBUF"])`
- 1 new integration test, 47 total, all passing
- Epic 1 complete (stories 1.1–1.7 all done)

### File List

**New:**
- `tests/api_list_schema_types.rs`

**Modified:**
- `src/api/mod.rs` — added route
- `src/api/schemas.rs` — added `list_types` handler
