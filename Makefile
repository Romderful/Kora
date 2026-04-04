.PHONY: dev test lint down clean

include .env
export

pg_ready = docker compose exec -T postgres pg_isready -U $(POSTGRES_USER) > /dev/null 2>&1
pg_up    = docker compose ps postgres --format '{{.State}}' 2>/dev/null | grep -q running

define ensure_pg
	$(pg_ready) || { docker compose up -d postgres; \
	  echo "Waiting for PG..."; until $(pg_ready); do sleep 0.3; done; }
endef

## Start PG + run Kora
dev:
	@$(call ensure_pg)
	DATABASE_URL=$(DATABASE_URL) cargo run

## Run tests — stops PG only if it wasn't already running
test:
	@was_up=false; $(pg_up) && was_up=true || true; \
	$(call ensure_pg); \
	DATABASE_URL=$(DATABASE_URL) cargo test -- --include-ignored; rc=$$?; \
	$$was_up || docker compose down; exit $$rc

## Clippy pedantic + deny warnings
lint:
	cargo clippy -- -D clippy::all -D clippy::pedantic

## Stop containers
down:
	docker compose down

## Stop containers + remove volumes
clean:
	docker compose down -v
