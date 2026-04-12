-- Initial Kora schema
-- Tables: subjects, schemas, schema_references, config

CREATE TABLE IF NOT EXISTS subjects (
    id         BIGSERIAL PRIMARY KEY,
    name       TEXT UNIQUE NOT NULL,
    deleted    BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS schemas (
    id             BIGSERIAL PRIMARY KEY,
    subject_id     BIGINT NOT NULL REFERENCES subjects(id),
    version        INT NOT NULL CHECK (version > 0),
    schema_type    TEXT NOT NULL DEFAULT 'AVRO',
    schema_text    TEXT NOT NULL,
    canonical_form TEXT,
    fingerprint    TEXT,
    raw_fingerprint TEXT NOT NULL DEFAULT '',
    deleted        BOOLEAN NOT NULL DEFAULT false,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (subject_id, version)
);

CREATE TABLE IF NOT EXISTS schema_references (
    id        BIGSERIAL PRIMARY KEY,
    schema_id BIGINT NOT NULL REFERENCES schemas(id),
    name      TEXT NOT NULL,
    subject   TEXT NOT NULL,
    version   INT NOT NULL
);

CREATE TABLE IF NOT EXISTS config (
    id                  BIGSERIAL PRIMARY KEY,
    subject             TEXT UNIQUE,
    compatibility_level TEXT NOT NULL DEFAULT 'BACKWARD',
    normalize           BOOLEAN NOT NULL DEFAULT false,
    mode                TEXT NOT NULL DEFAULT 'READWRITE',
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_schemas_subject_version ON schemas(subject_id, version);
CREATE INDEX IF NOT EXISTS idx_schemas_subject_fingerprint ON schemas(subject_id, fingerprint);
CREATE INDEX IF NOT EXISTS idx_schemas_subject_raw_fingerprint ON schemas(subject_id, raw_fingerprint);
CREATE INDEX IF NOT EXISTS idx_subjects_name ON subjects(name);
CREATE INDEX IF NOT EXISTS idx_schema_references_schema_id ON schema_references(schema_id);

-- Ensure at most one global config row (NULL subject).
-- PostgreSQL treats NULLs as distinct in UNIQUE constraints, so the column-level
-- UNIQUE alone does not prevent duplicate NULL rows.
CREATE UNIQUE INDEX IF NOT EXISTS idx_config_global ON config ((true)) WHERE subject IS NULL;

-- Global config row (subject = NULL means global)
INSERT INTO config (subject, compatibility_level, mode)
VALUES (NULL, 'BACKWARD', 'READWRITE')
ON CONFLICT DO NOTHING;
