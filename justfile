set dotenv-load

pg_ready := "docker compose exec -T postgres pg_isready -U $POSTGRES_USER > /dev/null 2>&1"

[private]
ensure-pg:
    @{{ pg_ready }} || { docker compose up -d postgres; \
      echo "Waiting for PG..."; until {{ pg_ready }}; do sleep 0.3; done; }

# Build and run everything in Docker
up:
    docker compose up --build -d

# Run all tests
test:
    #!/usr/bin/env bash
    set -euo pipefail
    was_up=false; docker compose ps api --format '{{{{.State}}}}' 2>/dev/null | grep -q running && was_up=true || true
    just ensure-pg
    cargo test --test '*' -- --include-ignored; rc=$?
    $was_up || docker compose down
    exit $rc

# Clippy + deny warnings
lint:
    cargo clippy -- -D clippy::all -D clippy::pedantic

# Stop containers
down:
    docker compose down

# Stop containers + remove volumes
clean:
    docker compose down -v
