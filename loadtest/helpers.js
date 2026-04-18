// helpers.js — Shared fixtures, deterministic generators, and tagged HTTP helpers for Kora load tests.

import http from 'k6/http';
import { SharedArray } from 'k6/data';

// -- Configuration --

export const BASE = __ENV.KORA_URL || 'http://localhost:8080';
export const HEADERS = { 'Content-Type': 'application/vnd.schemaregistry.v1+json' };

// -- Schema fixtures (SharedArray — zero-copy across VUs) --

// Base corpus: 500 Avro + 200 JSON Schema + 100 Protobuf subjects with v1 schemas.
// Loaded once at init, shared read-only across all VUs.
export const SCHEMAS = new SharedArray('schemas', function () {
  const arr = [];

  // 60% Avro (subjects 0–499)
  for (let i = 0; i < 500; i++) {
    arr.push({
      subject: `avro-perf-${i}`,
      schemaType: 'AVRO',
      schema: JSON.stringify({
        type: 'record',
        name: `AvroRec${i}`,
        namespace: 'kora.perf',
        fields: [
          { name: 'id', type: 'long' },
          { name: `field_${i}`, type: 'string' },
        ],
      }),
    });
  }

  // 25% JSON Schema (subjects 500–699)
  for (let i = 0; i < 200; i++) {
    arr.push({
      subject: `json-perf-${i}`,
      schemaType: 'JSON',
      schema: JSON.stringify({
        type: 'object',
        properties: {
          id: { type: 'integer' },
          [`field_${i}`]: { type: 'string' },
        },
        required: ['id'],
      }),
    });
  }

  // 15% Protobuf (subjects 700–799)
  for (let i = 0; i < 100; i++) {
    arr.push({
      subject: `proto-perf-${i}`,
      schemaType: 'PROTOBUF',
      schema: `syntax = "proto3";\nmessage ProtoRec${i} {\n  int64 id = 1;\n  string field_${i} = 2;\n}`,
    });
  }

  return arr;
});

// -- Deterministic schema generators (for VU-level variation without SharedArray bloat) --

export function evolveAvro(baseName, fieldName) {
  return JSON.stringify({
    type: 'record',
    name: baseName,
    namespace: 'kora.perf',
    fields: [
      { name: 'id', type: 'long' },
      { name: fieldName, type: ['null', 'string'], default: null },
    ],
  });
}

// -- Tagged HTTP helpers --

export function registerSchema(subject, schema, schemaType = 'AVRO') {
  return http.post(
    `${BASE}/subjects/${subject}/versions`,
    JSON.stringify({ schema, schemaType }),
    { headers: HEADERS, tags: { op: 'register', name: 'POST /subjects/{subject}/versions' } },
  );
}

export function getById(id) {
  return http.get(`${BASE}/schemas/ids/${id}`, {
    tags: { op: 'get_by_id', name: 'GET /schemas/ids/{id}' },
  });
}

export function getByVersion(subject, version = 'latest') {
  return http.get(`${BASE}/subjects/${subject}/versions/${version}`, {
    tags: { op: 'get_by_version', name: 'GET /subjects/{subject}/versions/{version}' },
  });
}

export function listSubjects(prefix = '') {
  const qs = prefix ? `?subjectPrefix=${prefix}` : '';
  return http.get(`${BASE}/subjects${qs}`, {
    tags: { op: 'list_subjects', name: 'GET /subjects' },
  });
}

export function listVersions(subject) {
  return http.get(`${BASE}/subjects/${subject}/versions`, {
    tags: { op: 'list_versions', name: 'GET /subjects/{subject}/versions' },
  });
}

export function checkSchema(subject, schema, schemaType = 'AVRO') {
  return http.post(
    `${BASE}/subjects/${subject}`,
    JSON.stringify({ schema, schemaType }),
    { headers: HEADERS, tags: { op: 'check_schema', name: 'POST /subjects/{subject}' } },
  );
}

export function testCompatibility(subject, version, schema, schemaType = 'AVRO') {
  return http.post(
    `${BASE}/compatibility/subjects/${subject}/versions/${version}`,
    JSON.stringify({ schema, schemaType }),
    { headers: HEADERS, tags: { op: 'compat', name: 'POST /compatibility/subjects/{subject}/versions/{version}' } },
  );
}

export function deleteSubject(subject, permanent = false) {
  const qs = permanent ? '?permanent=true' : '';
  return http.del(`${BASE}/subjects/${subject}${qs}`, null, {
    tags: { op: 'delete', name: 'DELETE /subjects/{subject}' },
  });
}

export function deleteVersion(subject, version, permanent = false) {
  const qs = permanent ? '?permanent=true' : '';
  return http.del(`${BASE}/subjects/${subject}/versions/${version}${qs}`, null, {
    tags: { op: 'delete', name: 'DELETE /subjects/{subject}/versions/{version}' },
  });
}

export function scrapeMetrics() {
  return http.get(`${BASE}/metrics`, {
    tags: { op: 'prom_scrape', name: 'GET /metrics' },
  });
}

// -- Seed helper (used in setup()) --

export function seedSchemas(count = 500) {
  const ids = [];
  const limit = Math.min(count, SCHEMAS.length);
  for (let i = 0; i < limit; i++) {
    const s = SCHEMAS[i];
    const res = registerSchema(s.subject, s.schema, s.schemaType);
    if (res.status === 200) {
      const body = res.json();
      ids.push({ id: body.id, subject: s.subject, schemaType: s.schemaType });
    }
  }
  return ids;
}
