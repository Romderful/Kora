-- Initial Kora schema
-- Tables: subjects, schemas, schema_references, config

CREATE TABLE subjects (
    id         BIGSERIAL PRIMARY KEY,
    name       TEXT UNIQUE NOT NULL,
    deleted    BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE schemas (
    id             BIGSERIAL PRIMARY KEY,
    subject_id     BIGINT NOT NULL REFERENCES subjects(id),
    version        INT NOT NULL CHECK (version > 0),
    schema_type    TEXT NOT NULL DEFAULT 'AVRO',
    schema_text    TEXT NOT NULL,
    canonical_form TEXT,
    fingerprint    TEXT,
    deleted        BOOLEAN NOT NULL DEFAULT false,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (subject_id, version)
);

CREATE TABLE schema_references (
    id        BIGSERIAL PRIMARY KEY,
    schema_id BIGINT NOT NULL REFERENCES schemas(id),
    name      TEXT NOT NULL,
    subject   TEXT NOT NULL,
    version   INT NOT NULL
);

CREATE TABLE config (
    id                  BIGSERIAL PRIMARY KEY,
    subject             TEXT UNIQUE,
    compatibility_level TEXT NOT NULL DEFAULT 'BACKWARD',
    mode                TEXT NOT NULL DEFAULT 'READWRITE',
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Indexes
CREATE INDEX idx_schemas_subject_version ON schemas(subject_id, version);
CREATE INDEX idx_subjects_name ON subjects(name);
CREATE INDEX idx_schema_references_schema_id ON schema_references(schema_id);

-- Global config row (subject = NULL means global)
INSERT INTO config (subject, compatibility_level, mode)
VALUES (NULL, 'BACKWARD', 'READWRITE');
