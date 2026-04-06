.PHONY: dev test test-unit test-integration lint migrate down clean

include .env
export

pg_ready = docker compose exec -T postgres pg_isready -U $(POSTGRES_USER) > /dev/null 2>&1
pg_up    = docker compose ps postgres --format '{{.State}}' 2>/dev/null | grep -q running

define ensure_pg
	$(pg_ready) || { docker compose up -d postgres; \
	  echo "Waiting for PG..."; until $(pg_ready); do sleep 0.3; done; }
endef

## Apply all migrations so sqlx macros can verify queries at compile time
migrate:
	@$(call ensure_pg)
	@ls migrations/*.sql | sort | xargs cat | docker compose exec -T postgres psql -q -U $(POSTGRES_USER) -d $(POSTGRES_DB) 2>/dev/null || true

# Discover test crates dynamically from tests/ filenames.
# Convention: api_* and db_* need PG, everything else is unit-only.
integration_tests = $(basename $(notdir $(wildcard tests/api_*.rs tests/db_*.rs)))
unit_tests        = $(filter-out $(integration_tests),$(basename $(notdir $(wildcard tests/*.rs))))

integration_flags = $(addprefix --test ,$(integration_tests))
unit_flags        = $(addprefix --test ,$(unit_tests))
all_flags         = $(unit_flags) $(integration_flags)

## Start PG + run Kora
dev: migrate
	DATABASE_URL=$(DATABASE_URL) cargo run

## Run all tests — stops PG only if it wasn't already running
test:
	@was_up=false; $(pg_up) && was_up=true || true; \
	$(MAKE) migrate; \
	DATABASE_URL=$(DATABASE_URL) cargo test $(all_flags) -- --include-ignored; rc=$$?; \
	$$was_up || docker compose down; exit $$rc

## Unit tests only (no PG needed)
test-unit:
	cargo test $(unit_flags)

## Integration tests only (needs PG)
test-integration:
	@was_up=false; $(pg_up) && was_up=true || true; \
	$(MAKE) migrate; \
	DATABASE_URL=$(DATABASE_URL) cargo test $(integration_flags) -- --include-ignored; rc=$$?; \
	$$was_up || docker compose down; exit $$rc

## Clippy pedantic + deny warnings
lint: migrate
	cargo clippy -- -D clippy::all -D clippy::pedantic

## Stop containers
down:
	docker compose down

## Stop containers + remove volumes
clean:
	docker compose down -v
