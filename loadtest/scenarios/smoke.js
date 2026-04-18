// smoke.js — Baseline validation: 1 VU, 30s.
// Runs the complete user journey to verify all endpoints work and establish baseline latencies.
// Run 3 times before setting thresholds for other scenarios.

import { check, sleep } from 'k6';
import {
  registerSchema, getById, getByVersion, listSubjects, listVersions,
  checkSchema, testCompatibility, evolveAvro, scrapeMetrics, BASE, HEADERS,
} from '../helpers.js';

export const options = {
  vus: 1,
  duration: '30s',
  setupTimeout: '30s',
  thresholds: {
    http_req_failed: ['rate==0'],
    http_req_duration: ['p(95)<500'],
    'http_req_duration{op:get_by_id}': ['p(95)<100'],
    'http_req_duration{op:get_by_version}': ['p(95)<100'],
    'http_req_duration{op:register}': ['p(95)<300'],
    'http_req_duration{op:compat}': ['p(95)<300'],
    'http_req_duration{op:list_subjects}': ['p(95)<200'],
    'http_req_duration{op:check_schema}': ['p(95)<200'],
  },
};

export function setup() {
  // Register a few schemas to have data for the journey.
  const ids = [];
  for (let i = 0; i < 10; i++) {
    const subject = `smoke-avro-${i}`;
    const schema = JSON.stringify({
      type: 'record', name: `SmokeRec${i}`, namespace: 'kora.smoke',
      fields: [{ name: 'id', type: 'long' }, { name: `f${i}`, type: 'string' }],
    });
    const res = registerSchema(subject, schema);
    if (res.status === 200) {
      ids.push({ id: res.json().id, subject, schema, schemaType: 'AVRO' });
    }
  }

  // Register a JSON Schema subject.
  const jsonSubject = 'smoke-json-0';
  const jsonSchema = JSON.stringify({
    type: 'object',
    properties: { id: { type: 'integer' }, name: { type: 'string' } },
    required: ['id'],
  });
  const jsonRes = registerSchema(jsonSubject, jsonSchema, 'JSON');
  if (jsonRes.status === 200) {
    ids.push({ id: jsonRes.json().id, subject: jsonSubject, schema: jsonSchema, schemaType: 'JSON' });
  }

  return { seeded: ids };
}

export default function (data) {
  const entry = data.seeded[Math.floor(Math.random() * data.seeded.length)];

  // 1. Lookup by global ID
  check(getById(entry.id), {
    'get_by_id: status 200': (r) => r.status === 200,
  });

  // 2. Lookup by version (latest)
  check(getByVersion(entry.subject), {
    'get_by_version: status 200': (r) => r.status === 200,
  });

  // 3. List subjects
  check(listSubjects(), {
    'list_subjects: status 200': (r) => r.status === 200,
  });

  // 4. List versions
  check(listVersions(entry.subject), {
    'list_versions: status 200': (r) => r.status === 200,
  });

  // 5. Check if schema is registered
  check(checkSchema(entry.subject, entry.schema, entry.schemaType), {
    'check_schema: status 200': (r) => r.status === 200,
  });

  // 6. Compatibility check (evolve the schema with a nullable field)
  if (entry.schemaType === 'AVRO') {
    const evolved = evolveAvro(`SmokeRec${0}`, `extra_${__ITER}`);
    check(testCompatibility(entry.subject, 'latest', evolved), {
      'compat: status 200': (r) => r.status === 200,
    });
  }

  // 7. Register a new unique schema (write path)
  const newSubject = `smoke-write-${__VU}-${__ITER}`;
  const newSchema = JSON.stringify({
    type: 'record', name: `W${__VU}_${__ITER}`, namespace: 'kora.smoke',
    fields: [{ name: 'id', type: 'long' }],
  });
  check(registerSchema(newSubject, newSchema), {
    'register: status 200': (r) => r.status === 200,
  });

  // 8. Scrape Prometheus metrics
  check(scrapeMetrics(), {
    'metrics: status 200': (r) => r.status === 200,
  });

  sleep(0.5);
}
