#!/bin/sh
set -e

if [ -z "$DATABASE_URL" ]; then
    if ! command -v initdb > /dev/null 2>&1; then
        echo "ERROR: DATABASE_URL is required (image built without embedded PostgreSQL)" >&2
        exit 1
    fi

    PGDATA="/var/lib/postgresql/data"

    if [ ! -f "$PGDATA/PG_VERSION" ]; then
        su-exec postgres initdb -D "$PGDATA" --auth=trust
    fi

    su-exec postgres pg_ctl start -D "$PGDATA" -l /var/lib/postgresql/pg.log -w -t 30 \
        || { echo "ERROR: PostgreSQL failed to start"; cat /var/lib/postgresql/pg.log >&2; exit 1; }

    su-exec postgres pg_isready -t 5 || { echo "ERROR: PostgreSQL not accepting connections" >&2; exit 1; }

    su-exec postgres createuser kora 2>/dev/null || true
    su-exec postgres createdb -O kora kora 2>/dev/null || true

    export DATABASE_URL="postgres://kora@localhost:5432/kora"

    # Stop PG gracefully on signal or when kora exits.
    cleanup() { su-exec postgres pg_ctl stop -D "$PGDATA" -m fast -w 2>/dev/null || true; }
    trap 'kill -TERM $KORA_PID 2>/dev/null; wait $KORA_PID; cleanup; exit' TERM INT QUIT

    /usr/local/bin/kora "$@" &
    KORA_PID=$!
    wait $KORA_PID
    EXIT_CODE=$?
    cleanup
    exit $EXIT_CODE
fi

exec /usr/local/bin/kora "$@"
