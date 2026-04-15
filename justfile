set dotenv-load

image     := env("KORA_IMAGE", "ghcr.io/romderful/kora")
platforms := "linux/amd64,linux/arm64"
pg_ready := "docker compose exec -T postgres pg_isready -U $POSTGRES_USER > /dev/null 2>&1"
db_ready := "docker compose exec -T postgres psql -U $POSTGRES_USER -d $POSTGRES_DB -c 'SELECT 1' > /dev/null 2>&1"

[private]
ensure-pg:
    @{{ pg_ready }} || { docker compose up -d postgres; \
      echo "Waiting for PG..."; until {{ pg_ready }}; do sleep 0.3; done; }
    @until {{ db_ready }}; do sleep 0.3; done

# ---------- Quality ----------

# Check formatting
[group('quality')]
fmt:
    cargo fmt --check

# Run clippy lints
[group('quality')]
lint:
    cargo clippy -- -D clippy::all -D clippy::pedantic

# Auto-fix formatting + clippy suggestions
[group('quality')]
fix:
    cargo fmt
    cargo clippy --fix --allow-dirty -- -D clippy::all -D clippy::pedantic

# ---------- Development ----------

# Run Kora locally with cargo (starts PG automatically)
[group('dev')]
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    just ensure-pg
    trap 'docker compose down' EXIT
    cargo run

# Run all tests (starts PG if needed, tears down after)
[group('dev')]
test:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "${CI:-}" = "true" ]; then
      echo "CI — PG managed by service container"
    elif pg_isready -h localhost -q 2>/dev/null; then
      echo "PG already running — skipping docker compose"
    else
      just ensure-pg
      trap 'docker compose down' EXIT
    fi
    cargo test --test '*' -- --include-ignored

# fmt + lint + test (CI entrypoint)
[group('quality')]
ci: fmt lint test

# ---------- Build & Push ----------

# Build + push slim image (amd64 + arm64)
[group('build')]
build tag="latest":
    docker buildx build --platform {{ platforms }} --provenance=false --build-arg EMBEDDED_PG=false -t {{ image }}:{{ tag }} --push .

# Build + push all-in-one image (amd64 + arm64)
[group('build')]
build-embedded tag="latest-embedded":
    docker buildx build --platform {{ platforms }} --provenance=false -t {{ image }}:{{ tag }} --push .

# Build + push both images (slim last → featured on ghcr.io)
[group('build')]
release tag="latest":
    just build-embedded {{ tag }}-embedded
    just build {{ tag }}

# ---------- Docker (local) ----------

# Run slim image locally (needs DATABASE_URL)
[group('docker')]
run db_url:
    docker run --rm --network host --name kora -e "DATABASE_URL={{ db_url }}" {{ image }}:latest

# Run all-in-one image locally
[group('docker')]
run-embedded:
    docker run --rm -p 8080:8080 --name kora {{ image }}:latest-embedded

# Stop Kora and compose services
[group('docker')]
stop:
    -docker stop kora
    -docker compose down

# Remove all containers, images, and volumes
[group('docker')]
clean:
    -docker rm -f kora
    -docker rmi {{ image }}:latest {{ image }}:latest-embedded
    -docker compose down -v
