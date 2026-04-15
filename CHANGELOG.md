# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/Romderful/Kora/releases/tag/v0.1.0) - 2026-04-15

### Added

- CI/CD pipeline — GitHub Actions + release-plz automated releases
- story 6.3 — Docker all-in-one packaging (embedded PG, multi-arch, ghcr.io registry)
- story 6.2 — Prometheus metrics endpoint (GET /metrics, request instrumentation, business gauges)
- story 6.1 — registry mode control (GET/PUT/DELETE /mode, enforcement, recursive delete)
- story 5.2 — enforce all 7 compatibility modes on schema registration + Confluent API wire parity
- story 5.1 — compatibility test endpoint + Confluent-compatible JSON Schema & Protobuf diff engines
- story 4.5 — referenced-by lookup (GET /subjects/{subject}/versions/{version}/referencedby)
- story 4.4 — list all schemas + schema text endpoints (GET /schemas, /schema)
- story 4.3 — Confluent API parity list & lookup params (subjectPrefix, deletedOnly, deletedAsNegative, normalize, deleted on cross-refs/check/get-by-version)
- story 4.2 — Confluent API parity infrastructure (GET/POST /, error codes, pagination, config params)
- story 4.1 — compatibility configuration CRUD (GET/PUT /config, GET/PUT/DELETE /config/{subject})
- story 3.2 — Protobuf format handler (POST schemaType=PROTOBUF, parse, validate, fingerprint)
- story 3.1 — JSON Schema format handler (POST schemaType=JSON, parse, validate, fingerprint)
- story 2.4 — schema references and dependency protection (POST refs, hard-delete guard)
- story 2.3 — schema ID cross-references (GET /schemas/ids/{id}/subjects, /versions)
- story 2.2 — hard-delete subject and versions (DELETE ?permanent=true)
- story 2.1 — soft-delete subject and versions (DELETE /subjects, DELETE /subjects/{subject}/versions)
- story 1.7 — list supported schema types (GET /schemas/types)
- story 1.6 — check schema registration (POST /subjects/{subject})
- story 1.5 — list subjects and versions (GET /subjects, GET /subjects/{subject}/versions)
- story 1.4 — retrieve schema by subject and version (GET /subjects/{subject}/versions/{version})
- story 1.3 — retrieve schema by global ID (GET /schemas/ids/{id})
- story 1.2 — register Avro schema (POST /subjects/{subject}/versions)
- story 1.1 — project scaffold, database & health check

### Other

- global schema dedup — split schemas into schema_contents + schema_versions
- restructure epics for 100% Confluent API wire-compatibility
- standardize test naming convention across all 15 test modules
- dockerize app, replace Make with Just, simplify sqlx usage
- add README, simplify config to env-only, remove TOML support
- add Kora epics & stories — 6 epics, 22 stories, 49 FRs mapped
- add Kora architecture — Rust/axum/sqlx stack, patterns, validation
- add Kora PRD — 49 FRs, 16 NFRs, Confluent API v7 spec
- add Kora product brief
- Initial project setup with BMad method
