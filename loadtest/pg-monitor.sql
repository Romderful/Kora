-- pg-monitor.sql — Run these queries during load tests for PostgreSQL-side observability.
-- Usage: psql -h localhost -p 5433 -U kora -d kora_loadtest -f pg-monitor.sql

-- Enable pg_stat_statements (once per database)
CREATE EXTENSION IF NOT EXISTS pg_stat_statements;

-- 1. Connection state
SELECT state, count(*)
FROM pg_stat_activity
WHERE datname = current_database()
GROUP BY state;

-- 2. Lock contention (waiting locks)
SELECT relation::regclass, mode, count(*)
FROM pg_locks
WHERE NOT granted
GROUP BY 1, 2;

-- 3. Blocked queries (who is blocking whom)
SELECT
  blocked.pid AS blocked_pid,
  blocked.query AS blocked_query,
  blocking.pid AS blocking_pid,
  blocking.query AS blocking_query
FROM pg_stat_activity blocked
JOIN pg_locks bl ON bl.pid = blocked.pid AND NOT bl.granted
JOIN pg_locks kl ON kl.locktype = bl.locktype
  AND kl.relation = bl.relation
  AND kl.granted
JOIN pg_stat_activity blocking ON blocking.pid = kl.pid
WHERE blocked.pid != blocking.pid;

-- 4. Table stats (dead tuples, vacuum lag, scan types)
SELECT
  relname,
  n_live_tup,
  n_dead_tup,
  ROUND(100.0 * n_dead_tup / NULLIF(n_live_tup + n_dead_tup, 0), 1) AS dead_pct,
  seq_scan,
  idx_scan,
  last_autovacuum,
  last_autoanalyze
FROM pg_stat_user_tables
ORDER BY n_dead_tup DESC;

-- 5. Long-running transactions
SELECT
  pid,
  age(now(), xact_start) AS tx_age,
  state,
  LEFT(query, 80) AS query
FROM pg_stat_activity
WHERE xact_start IS NOT NULL
  AND state != 'idle'
ORDER BY xact_start;

-- 6. Top queries by total time (pg_stat_statements)
SELECT
  LEFT(query, 100) AS query,
  calls,
  ROUND(total_exec_time::numeric, 2) AS total_ms,
  ROUND(mean_exec_time::numeric, 2) AS mean_ms,
  ROUND(stddev_exec_time::numeric, 2) AS stddev_ms,
  rows
FROM pg_stat_statements
WHERE dbid = (SELECT oid FROM pg_database WHERE datname = current_database())
ORDER BY total_exec_time DESC
LIMIT 20;

-- 7. Buffer hit ratio (should be > 99% for a warm cache)
SELECT
  ROUND(100.0 * sum(blks_hit) / NULLIF(sum(blks_hit) + sum(blks_read), 0), 2) AS buffer_hit_pct
FROM pg_stat_database
WHERE datname = current_database();
