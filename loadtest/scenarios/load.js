// load.js — Nominal production load with named k6 scenarios.
// Models the real user journeys: reads (high volume), writes (steady rate), compat checks (constant).
// Each flow has independent VU allocation and thresholds.

import { check, sleep } from 'k6';
import {
  seedSchemas, getById, getByVersion, listSubjects, checkSchema,
  registerSchema, testCompatibility, evolveAvro, scrapeMetrics,
  SCHEMAS,
} from '../helpers.js';

export const options = {
  setupTimeout: '120s',
  scenarios: {
    // Read-heavy: simulates schema lookups (the dominant real-world pattern).
    readers: {
      executor: 'ramping-vus',
      exec: 'readFlow',
      startVUs: 0,
      stages: [
        { duration: '30s', target: 50 },
        { duration: '4m', target: 50 },
        { duration: '30s', target: 0 },
      ],
    },

    // Writes: constant arrival rate (independent of VU think time).
    writers: {
      executor: 'constant-arrival-rate',
      exec: 'writeFlow',
      rate: 20,
      timeUnit: '1s',
      duration: '5m',
      preAllocatedVUs: 10,
      maxVUs: 30,
    },

    // Compatibility checks: steady load (CPU-bound path).
    compat_checkers: {
      executor: 'constant-vus',
      exec: 'compatFlow',
      vus: 5,
      duration: '5m',
    },

    // Check-schema: verify if schema is registered (distinct read path).
    check_schema: {
      executor: 'constant-vus',
      exec: 'checkSchemaFlow',
      vus: 3,
      duration: '5m',
    },

    // Prometheus scraper: low-rate, excluded from thresholds.
    prom_scrape: {
      executor: 'constant-arrival-rate',
      exec: 'scrapeFlow',
      rate: 1,
      timeUnit: '10s',
      duration: '5m',
      preAllocatedVUs: 1,
    },
  },

  thresholds: {
    // Global
    http_req_failed: ['rate<0.01'],

    // Per operation
    'http_req_duration{op:get_by_id}': ['p(95)<50', 'p(99)<150'],
    'http_req_duration{op:get_by_version}': ['p(95)<50', 'p(99)<150'],
    'http_req_duration{op:list_subjects}': ['p(95)<100', 'p(99)<300'],
    'http_req_duration{op:register}': ['p(95)<200', 'p(99)<500'],
    'http_req_duration{op:compat}': ['p(95)<200', 'p(99)<500'],
    'http_req_duration{op:check_schema}': ['p(95)<100', 'p(99)<300'],
  },
};

export function setup() {
  const seeded = seedSchemas(500);
  return { seeded };
}

// -- Scenario exec functions --

export function readFlow(data) {
  const entry = data.seeded[Math.floor(Math.random() * data.seeded.length)];

  check(getById(entry.id), {
    'read: get_by_id 200': (r) => r.status === 200,
  });

  check(getByVersion(entry.subject), {
    'read: get_by_version 200': (r) => r.status === 200,
  });

  // 20% of reads also list subjects (discovery pattern).
  if (Math.random() < 0.2) {
    check(listSubjects(), {
      'read: list_subjects 200': (r) => r.status === 200,
    });
  }

  sleep(0.1);
}

export function writeFlow() {
  // Each write creates a unique subject to avoid idempotency.
  const idx = Math.floor(Math.random() * SCHEMAS.length);
  const base = SCHEMAS[idx];
  const uniqueSubject = `load-w-${base.subject}-${Date.now()}-${Math.floor(Math.random() * 10000)}`;

  check(registerSchema(uniqueSubject, base.schema, base.schemaType), {
    'write: register 200': (r) => r.status === 200,
  });
  // No sleep — arrival-rate executor controls pacing.
}

export function compatFlow(data) {
  const entry = data.seeded[Math.floor(Math.random() * data.seeded.length)];

  // Only test Avro compat (JSON/Proto have different evolution rules, keep it simple).
  if (entry.schemaType !== 'AVRO') {
    sleep(0.1);
    return;
  }

  const evolved = evolveAvro(
    entry.subject.replace('avro-perf-', 'AvroRec'),
    `compat_${__VU}_${__ITER}`,
  );

  check(testCompatibility(entry.subject, 'latest', evolved), {
    'compat: status 200': (r) => r.status === 200,
  });

  sleep(0.5);
}

export function checkSchemaFlow(data) {
  const entry = data.seeded[Math.floor(Math.random() * data.seeded.length)];
  const base = SCHEMAS.find((s) => s.subject === entry.subject);
  if (!base) {
    sleep(0.1);
    return;
  }

  check(checkSchema(entry.subject, base.schema, base.schemaType), {
    'check: status 200': (r) => r.status === 200,
  });

  sleep(0.3);
}

export function scrapeFlow() {
  scrapeMetrics();
}
