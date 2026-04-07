# Kora

A schema registry built in Rust, wire-compatible with the Confluent Schema Registry API.

Kora stores schemas in PostgreSQL instead of Kafka, ships as a single binary, and delivers sub-millisecond lookups with zero JVM overhead.

## What it does

Kora manages schema definitions for Kafka ecosystems. Producers register schemas, consumers resolve them by ID, and the registry enforces compatibility rules to prevent breaking changes from reaching production.

It implements the full Confluent Schema Registry REST API, so existing Kafka serializers, connectors, and CLI tools work without modification — just point them at Kora instead.

## Why not Confluent or Karapace?

**Confluent Schema Registry** stores schemas in a Kafka topic. This creates a circular dependency: the tool that validates your Kafka data depends on Kafka itself. It runs on the JVM, which means GC pauses, warmup time, and tuning overhead.

**Karapace** (Aiven's Python alternative) solves the licensing problem but introduces Python runtime overhead and still stores schemas in Kafka.

**Kora** takes a different approach:

- **PostgreSQL storage** — schemas are regular database rows. Back up with `pg_dump`, query with SQL, integrate with your existing database tooling. No Kafka dependency.
- **Native performance** — compiled Rust, no garbage collector, no interpreter. Schema lookups in microseconds.
- **Schema comparison** — a diff API that tells you exactly what changed between two schema versions, with typed change classifications and breaking-change verdicts. No other registry offers this.

## Supported formats

- **Avro** — full support including canonical form normalization and Rabin fingerprinting
- **JSON Schema** — validation and compatibility checking
- **Protobuf** — parsing, compatibility, and schema references

## Compatibility modes

Seven modes, configurable globally or per subject:

- **NONE** — accept anything
- **BACKWARD / BACKWARD_TRANSITIVE** — new schema can read old data
- **FORWARD / FORWARD_TRANSITIVE** — old schema can read new data
- **FULL / FULL_TRANSITIVE** — both directions

Default is BACKWARD, matching Confluent's behavior.

## How schemas work

A **subject** is a named container (typically `<topic>-value` or `<topic>-key`). Each schema registered under a subject gets a sequential **version** number and a globally unique **ID**.

The ID is permanent. Even if a schema is soft-deleted (hidden from subject listings), consumers can still resolve it by ID — because Kafka messages already in flight reference that ID and must remain deserializable.

## Configuration

Copy `.env.example` to `.env`. It contains the PostgreSQL credentials for Docker and the `DATABASE_URL` used by the application.

The server binds to `0.0.0.0:8080` by default. These defaults and a few others (`max_body_size`) can be overridden via environment variables if needed — see `src/config.rs`.

## Running

Requires [just](https://github.com/casey/just) and Docker.

```
just up                # build and run everything in Docker
```

This starts PostgreSQL, applies migrations, builds Kora, and launches it on port 8080.

## Development

```
just test              # all tests
just lint              # clippy pedantic
just down              # stop containers
just clean             # stop containers and remove volumes
```

The codebase enforces `deny(clippy::pedantic)` and `deny(missing_docs)`. All SQL queries are verified at compile time via sqlx macros.

## License

MIT
