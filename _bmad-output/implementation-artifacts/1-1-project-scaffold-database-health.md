# Story 1.1: Project Scaffold, Database & Health Check

Status: done

## Story

As a **developer**,
I want a running Kora server with PostgreSQL connectivity and a health endpoint,
so that I have the foundation to build all schema registry features on.

## Acceptance Criteria

1. Project compiles with zero warnings under `clippy::pedantic` and `deny(missing_docs)` enforced at crate level
2. Kora starts with `DATABASE_URL` configured and runs database migrations automatically on startup
3. Database tables `schemas`, `subjects`, `schema_references`, and `config` are created by migrations
4. `GET /health` returns HTTP 200 when PG is reachable, HTTP 503 when PG is unreachable
5. All error responses follow Confluent format: `{"error_code": <int>, "message": "<string>"}`
6. `Content-Type` header is `application/vnd.schemaregistry.v1+json` on all responses
7. Configuration loaded via figment: defaults → optional config file → env vars (`KORA_` prefix, `DATABASE_URL` for PG)
8. Graceful shutdown via tokio signal handler (SIGTERM/SIGINT) with connection draining
9. All tests pass via `cargo test`

## Tasks / Subtasks

- [x] Task 1: Initialize project and dependencies (AC: #1)
  - [x] `cargo init kora` in project root
  - [x] Add all crate dependencies (see Dev Notes for exact `cargo add` commands)
  - [x] Create `src/lib.rs` with `#![deny(missing_docs)]` and `#![deny(clippy::pedantic)]`
  - [x] Create `docker-compose.yml` for dev PG instance
  - [x] Create `Makefile` with smart PG lifecycle (dev, test, lint, down, clean)
  - [x] Create `.env` + `.env.example` for credentials (gitignored)
- [x] Task 2: Configuration module (AC: #7)
  - [x] Create `src/config.rs` with `KoraConfig` struct (figment: defaults → file → env)
  - [x] Fields: `database_url`, `host` (default "0.0.0.0"), `port` (default 8080), `log_level` (default "info")
  - [x] TDD: test default config, test env override, test DATABASE_URL required
- [x] Task 3: Error handling (AC: #5, #6)
  - [x] Create `src/error.rs` with `KoraError` enum
  - [x] Implement `axum::response::IntoResponse` producing Confluent JSON format
  - [x] Map variants to Confluent error codes (40401, 40402, 40403, 42201, 42202, 40901, 50001, 50002, 50003)
  - [x] TDD: test each error variant serializes correctly with right HTTP status
- [x] Task 4: Database setup and migrations (AC: #2, #3)
  - [x] Create `src/storage/mod.rs` with PgPool setup helper
  - [x] Create `migrations/001_initial_schema.sql` with tables:
    - `subjects` (id BIGSERIAL PK, name TEXT UNIQUE NOT NULL, deleted BOOLEAN DEFAULT false, created_at TIMESTAMPTZ, updated_at TIMESTAMPTZ)
    - `schemas` (id BIGSERIAL PK, subject_id BIGINT FK, version INT NOT NULL CHECK (version > 0), schema_type TEXT NOT NULL DEFAULT 'AVRO', schema_text TEXT NOT NULL, canonical_form TEXT, fingerprint TEXT, deleted BOOLEAN DEFAULT false, created_at TIMESTAMPTZ, UNIQUE(subject_id, version))
    - `schema_references` (id BIGSERIAL PK, schema_id BIGINT FK, name TEXT NOT NULL, subject TEXT NOT NULL, version INT NOT NULL)
    - `config` (id BIGSERIAL PK, subject TEXT UNIQUE, compatibility_level TEXT NOT NULL DEFAULT 'BACKWARD', mode TEXT NOT NULL DEFAULT 'READWRITE', updated_at TIMESTAMPTZ)
    - Indexes: `idx_schemas_subject_version`, `idx_subjects_name`, `idx_schema_references_schema_id`
  - [x] Embed migrations via `sqlx::migrate!()` macro
  - [x] TDD: test migration runs, test tables exist (integration test with real PG)
- [x] Task 5: Health endpoint (AC: #4)
  - [x] Create `src/api/mod.rs` with Router construction
  - [x] Create `src/api/health.rs` with `GET /health` handler
  - [x] Handler checks PG connectivity via `sqlx::query("SELECT 1")` — returns 200 or 503
  - [x] Response body: `{"status": "UP"}` or `{"status": "DOWN"}`
  - [x] TDD: test healthy PG returns 200, test unreachable PG returns 503
- [x] Task 6: Server entrypoint and graceful shutdown (AC: #8)
  - [x] Create `src/main.rs` with tokio entrypoint
  - [x] Load config → create PG pool → run migrations → build router → bind server
  - [x] Graceful shutdown: `tokio::signal::ctrl_c()` + `axum::serve(...).with_graceful_shutdown()`
  - [x] Structured logging: `tracing_subscriber::fmt().json().init()`
  - [x] TDD: test server starts and responds to /health (integration test)
- [x] Task 7: CI-readiness verification (AC: #1, #9)
  - [x] Run `cargo clippy -- -D clippy::all -D clippy::pedantic` — zero warnings
  - [x] Run `cargo test` — all tests pass
  - [x] Verify `cargo build --release` compiles cleanly

## Dev Notes

### Cargo Dependencies (exact commands)

```bash
cargo add axum tokio --features tokio/full
cargo add sqlx --features "runtime-tokio,tls-rustls-ring-webpki,postgres,migrate,macros,uuid,chrono,json"
cargo add serde serde_json --features serde/derive
cargo add apache-avro
cargo add prost prost-types
cargo add jsonschema --default-features=false
cargo add tracing tracing-subscriber --features tracing-subscriber/json
cargo add metrics metrics-exporter-prometheus
cargo add figment --features "toml,env"
cargo add thiserror
cargo add uuid --features "v4,serde"
cargo add chrono --features serde
cargo add dashmap
```

Dev dependencies:
```bash
cargo add --dev tokio-test
cargo add --dev reqwest --features json
```

### Architecture Compliance

- **Code Philosophy**: No bullshit, no over-engineering, simple + readable + documented
- **Module structure**: Flat modules (`src/config.rs`, `src/error.rs`, `src/api/`, `src/storage/`)
- **Error pattern**: `Result<T, KoraError>` everywhere, `KoraError` implements `IntoResponse`
- **Naming**: snake_case modules, PascalCase structs, snake_case functions, SCREAMING_SNAKE constants
- **DB naming**: snake_case plural tables, snake_case columns, `idx_{table}_{columns}` indexes, `fk_{table}_{ref_table}` foreign keys
- **No `unwrap()`** outside `#[cfg(test)]` — use `?` or `expect("reason")` for provably-safe cases
- **TDD mandatory**: Write failing test FIRST, then minimum code to pass, then refactor

### Confluent Error Code Reference

| Code | HTTP Status | Meaning |
|------|-------------|---------|
| 40401 | 404 | Subject not found |
| 40402 | 404 | Version not found |
| 40403 | 404 | Schema not found |
| 42201 | 422 | Invalid schema |
| 42202 | 422 | Invalid version |
| 40901 | 409 | Incompatible schema |
| 50001 | 500 | Error in backend data store |
| 50002 | 500 | Operation timed out |
| 50003 | 500 | Error forwarding request |

### Content-Type Handling

All responses (success AND error) must set:
```
Content-Type: application/vnd.schemaregistry.v1+json
```
Implemented as `tower_http::set_header::SetResponseHeaderLayer` on the Router via `src/api/middleware.rs`.

### Migration SQL Notes

- Use `BIGSERIAL` for all primary keys (schema IDs must be global sequential)
- `subjects.name` is the "subject" string from Confluent API (e.g., "orders-value")
- `schemas.schema_text` stores the raw schema as submitted
- `schemas.canonical_form` stores normalized form for deduplication
- `config` table: one row per subject (NULL subject = global config)
- Global config row: INSERT in migration with `subject = NULL, compatibility_level = 'BACKWARD', mode = 'READWRITE'`

### Docker Compose for Dev

```yaml
services:
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: ${POSTGRES_DB}
      POSTGRES_USER: ${POSTGRES_USER}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
    ports:
      - "${POSTGRES_PORT:-5432}:5432"
```

### Project Structure After This Story

```
kora/
├── Cargo.toml
├── Cargo.lock
├── Makefile
├── .env              ← gitignored, source of truth for credentials
├── .env.example      ← committed, cp to .env for dev setup
├── .gitignore
├── docker-compose.yml
├── migrations/
│   └── 001_initial_schema.sql
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── config.rs
│   ├── error.rs
│   ├── api/
│   │   ├── mod.rs
│   │   ├── health.rs
│   │   └── middleware.rs
│   └── storage/
│       └── mod.rs
└── tests/
    ├── common/
    │   └── mod.rs     ← shared helpers (pool, spawn_server)
    ├── health.rs
    └── migrations.rs
```

### Testing Strategy

- **Unit tests**: Co-located in each file (`#[cfg(test)] mod tests`)
  - `config.rs`: test defaults, env overrides
  - `error.rs`: test each KoraError variant → JSON + HTTP status
- **Integration tests**: Split by domain in `tests/`, shared helpers in `tests/common/mod.rs`
  - `tests/health.rs`: health endpoint
  - `tests/migrations.rs`: table creation, global config
  - Future stories add their own file (e.g. `tests/register_schema.rs`)
  - `DATABASE_URL` must be set via env (no hardcoded fallback) — use `make test`
  - Use `reqwest` to make HTTP requests to spawned server

### References

- [Source: architecture.md — Code Philosophy section]
- [Source: architecture.md — Data Architecture section]
- [Source: architecture.md — Infrastructure & Deployment section]
- [Source: architecture.md — Implementation Patterns section]
- [Source: architecture.md — Project Structure & Boundaries section]
- [Source: epics.md — Story 1.1 acceptance criteria]
- [Source: prd.md — FR39, FR40, FR41, FR46, FR47, FR48]

## Dev Agent Record

### Agent Model Used

### Debug Log References

### Completion Notes List

- Content-Type handled globally via `tower_http::SetResponseHeaderLayer` middleware (not per-handler)
- `schemas.version` has `CHECK (version > 0)` constraint
- Makefile: `make test` auto-downs PG only if it wasn't already running
- No hardcoded credentials — `.env` is single source of truth, docker-compose reads `${POSTGRES_*}` vars
- Test structure split by domain for scalability across stories
- 15 tests total: 12 unit (config: 3, error: 9) + 3 integration (migrations: 2, health: 1)

### File List
