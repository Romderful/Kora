<div align="center">

# Kora

**A Confluent-compatible Schema Registry, built in Rust.**

PostgreSQL storage · Single binary · Sub-millisecond lookups · Zero JVM overhead

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![ghcr.io](https://img.shields.io/badge/ghcr.io-romderful%2Fkora-blue?logo=docker)](https://github.com/Romderful/Kora/pkgs/container/kora)

</div>

---

## Why Kora?

| | Confluent | Karapace | Kora |
|---|---|---|---|
| **Storage** | Kafka topic | Kafka topic | PostgreSQL |
| **Runtime** | JVM | Python | Native (Rust) |
| **Kafka dependency** | Required | Required | None |
| **API compatibility** | Reference | Partial | 100% wire-compatible |

**Confluent Schema Registry** stores schemas in a Kafka topic — a circular dependency where the tool validating your Kafka data depends on Kafka itself. It runs on the JVM with GC pauses, warmup time, and tuning overhead.

**Karapace** (Aiven) solves the licensing issue but introduces Python runtime overhead and still depends on Kafka.

**Kora** stores schemas as regular PostgreSQL rows. Back up with `pg_dump`, query with SQL, integrate with your existing tooling. Every endpoint, query parameter, and error code matches the Confluent API — existing serializers, connectors, and CLI tools work without modification.

---

## Quick Start

### All-in-one (embedded PostgreSQL)

```bash
docker run -p 8080:8080 ghcr.io/romderful/kora:latest-embedded
```

That's it. PostgreSQL starts automatically inside the container. Data is ephemeral by default — add a volume for persistence:

```bash
docker run -p 8080:8080 -v kora-data:/var/lib/postgresql/data ghcr.io/romderful/kora:latest-embedded
```

> **Note:** The embedded image is designed for standalone, edge, and demo use cases. For production workloads, prefer the slim image with an external PostgreSQL — it gives you independent backups, scaling, and version upgrades.

### With an external PostgreSQL

```bash
docker run -p 8080:8080 -e DATABASE_URL="postgres://user:pass@host:5432/kora" ghcr.io/romderful/kora:latest
```

### Verify

```bash
curl http://localhost:8080/health
# {"status":"UP"}
```

---

## Images

Two multi-arch images (linux/amd64 + linux/arm64) are published to GitHub Container Registry:

| Image | Tag | Size | Use case |
|---|---|---|---|
| `ghcr.io/romderful/kora` | `latest` | ~24 MB | Production with external PostgreSQL |
| `ghcr.io/romderful/kora` | `latest-embedded` | ~73 MB | Standalone, edge, demos |

The `latest` (slim) image contains Kora, tini, and migrations. The `latest-embedded` image additionally bundles PostgreSQL 17, auto-initializes on first boot, and shuts down gracefully on `docker stop`.

---

## Configuration

All configuration is done via environment variables:

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` | — (required for slim) | PostgreSQL connection string |
| `HOST` | `0.0.0.0` | Server bind address |
| `PORT` | `8080` | Server listen port |
| `MAX_BODY_SIZE` | `16777216` | Maximum request body size (bytes, default 16 MB) |
| `DB_POOL_MAX` | `20` | Maximum database connections in the pool |
| `RUST_LOG` | `info` | Log level filter (`error`, `warn`, `info`, `debug`, `trace`) |

The embedded image auto-generates `DATABASE_URL` when none is provided. If `DATABASE_URL` is set, embedded PostgreSQL is skipped entirely — even on the embedded image.

---

## API

Kora implements the full [Confluent Schema Registry REST API](https://docs.confluent.io/platform/current/schema-registry/develop/api.html). All standard clients (Kafka serializers, `kafka-schema-registry-maven-plugin`, `schema-registry-cli`, etc.) work without modification.

### Schemas

```
POST   /subjects/{subject}/versions                           Register a schema
GET    /subjects/{subject}/versions/{version}                 Get schema by version
GET    /subjects/{subject}/versions                           List versions
DELETE /subjects/{subject}/versions/{version}                 Delete version (soft/hard)
DELETE /subjects/{subject}                                    Delete subject (soft/hard)
POST   /subjects/{subject}                                    Check if schema is registered
GET    /schemas/ids/{id}                                      Get schema by global ID
GET    /schemas/ids/{id}/schema                               Get raw schema text
GET    /schemas/ids/{id}/subjects                             Subjects using this schema
GET    /schemas/ids/{id}/versions                             Versions using this schema
GET    /subjects                                              List all subjects
GET    /schemas                                               List all schemas
GET    /schemas/types                                         List supported types
GET    /subjects/{subject}/versions/{version}/schema          Raw schema text
GET    /subjects/{subject}/versions/{version}/referencedby    Referenced-by IDs
```

### Compatibility

```
POST   /compatibility/subjects/{subject}/versions/{version}   Test against version
POST   /compatibility/subjects/{subject}/versions             Test against all versions
GET    /config                                                Get global compatibility
PUT    /config                                                Set global compatibility
DELETE /config                                                Reset global compatibility
GET    /config/{subject}                                      Get subject compatibility
PUT    /config/{subject}                                      Set subject compatibility
DELETE /config/{subject}                                      Reset subject compatibility
```

### Registry Mode

```
GET    /mode                                                  Get global mode
PUT    /mode                                                  Set global mode
DELETE /mode                                                  Reset global mode
GET    /mode/{subject}                                        Get subject mode
PUT    /mode/{subject}                                        Set subject mode
DELETE /mode/{subject}                                        Reset subject mode
```

### Operations

```
GET    /health                                                Health check
GET    /metrics                                               Prometheus metrics
GET    /                                                      Root (Confluent compat)
POST   /                                                      Root (Confluent compat)
```

---

## Schema Formats

| Format | Registration | Compatibility | References |
|---|---|---|---|
| **Avro** | Canonical form normalization, Rabin fingerprinting | All 7 modes | Supported |
| **JSON Schema** | Validation, fingerprinting | All 7 modes | Supported |
| **Protobuf** | Parsing, validation, fingerprinting | All 7 modes | Supported |

---

## Compatibility Modes

Seven modes, configurable globally or per subject:

| Mode | Direction | Scope |
|---|---|---|
| `NONE` | Accept anything | — |
| `BACKWARD` | New schema can read old data | Last version |
| `BACKWARD_TRANSITIVE` | New schema can read old data | All versions |
| `FORWARD` | Old schema can read new data | Last version |
| `FORWARD_TRANSITIVE` | Old schema can read new data | All versions |
| `FULL` | Both directions | Last version |
| `FULL_TRANSITIVE` | Both directions | All versions |

Default: `BACKWARD` (matches Confluent).

---

## Deployment

### Kubernetes (external PostgreSQL)

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: kora
spec:
  replicas: 3
  selector:
    matchLabels:
      app: kora
  template:
    metadata:
      labels:
        app: kora
    spec:
      containers:
        - name: kora
          image: ghcr.io/romderful/kora:latest
          ports:
            - containerPort: 8080
          env:
            - name: DATABASE_URL
              valueFrom:
                secretKeyRef:
                  name: kora-db
                  key: url
          readinessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 2
            periodSeconds: 5
          livenessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 5
            periodSeconds: 10
          resources:
            requests:
              cpu: 100m
              memory: 64Mi
            limits:
              cpu: 500m
              memory: 128Mi
```

### Docker Compose

```yaml
services:
  postgres:
    image: postgres:17-alpine
    environment:
      POSTGRES_DB: kora
      POSTGRES_USER: kora
      POSTGRES_PASSWORD: kora
    volumes:
      - pgdata:/var/lib/postgresql/data

  kora:
    image: ghcr.io/romderful/kora:latest
    depends_on:
      - postgres
    environment:
      DATABASE_URL: postgres://kora:kora@postgres:5432/kora
    ports:
      - "8080:8080"

volumes:
  pgdata:
```

### Standalone (embedded PostgreSQL)

```bash
# Ephemeral (data lost on container removal)
docker run -p 8080:8080 ghcr.io/romderful/kora:latest-embedded

# Persistent
docker run -p 8080:8080 -v kora-data:/var/lib/postgresql/data ghcr.io/romderful/kora:latest-embedded
```

---

## Development

Requires [just](https://github.com/casey/just), Rust, and Docker.

```
just dev       # Run locally with cargo (starts PG via Docker Compose)
just test      # Run all tests (starts PG automatically)
just fix       # Auto-fix formatting + clippy suggestions
just ci        # fmt + lint + test (same as CI)
just stop      # Stop all containers
just clean     # Remove containers, images, and volumes
```

### Build & Publish

```bash
just release             # Build + push both images (multi-arch)
just release v0.4.0      # Tagged release
just build               # Slim image only
just build-embedded      # Embedded image only
```

Override the registry: `KORA_IMAGE=my-registry.io/kora just release`

### All recipes

```
[build]    build, build-embedded, release          Build + push to ghcr.io (amd64 + arm64)
[dev]      dev, test                               Local development
[docker]   run, run-embedded, stop, clean          Run images locally
[loadtest] smoke, load, stress, soak, contention   k6 load tests (requires k6)
[quality]  fmt, lint, fix, ci                      Code quality + CI entrypoint
```

---

## Architecture

```
Client → Kora (Axum + Tokio) → PostgreSQL
                ↓
         Prometheus /metrics
```

- **Axum** HTTP framework with Tower middleware
- **SQLx** compile-time verified queries with connection pooling
- **Rustls** TLS (no OpenSSL dependency)
- **Static musl binary** — single file, no runtime dependencies
- **tini** PID 1 in Docker for proper signal handling

---

## License

MIT
