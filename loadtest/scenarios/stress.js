// stress.js — Find the breaking point.
// Ramps VUs aggressively to saturate the connection pool and find where latency goes non-linear.
// Run with different DB_POOL_MAX values (10, 20, 50) to measure pool impact.

import { check, sleep } from 'k6';
import {
  seedSchemas, getById, getByVersion, registerSchema, testCompatibility,
  evolveAvro, SCHEMAS,
} from '../helpers.js';

export const options = {
  setupTimeout: '120s',
  scenarios: {
    stress_mix: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '30s', target: 10 },   // warm-up
        { duration: '1m', target: 50 },     // moderate
        { duration: '1m', target: 100 },    // heavy
        { duration: '1m', target: 200 },    // extreme
        { duration: '1m', target: 300 },    // breaking point
        { duration: '1m', target: 300 },    // sustained peak
        { duration: '30s', target: 0 },     // cool-down
      ],
    },
  },

  thresholds: {
    // Relaxed thresholds — we expect degradation, we're finding limits.
    http_req_failed: ['rate<0.05'],
    'http_req_duration{op:get_by_id}': ['p(95)<500'],
    'http_req_duration{op:register}': ['p(95)<2000'],
  },
};

export function setup() {
  const seeded = seedSchemas(300);
  return { seeded };
}

export default function (data) {
  const roll = Math.random();

  if (roll < 0.60) {
    // 60% reads
    const entry = data.seeded[Math.floor(Math.random() * data.seeded.length)];
    check(getById(entry.id), {
      'stress: get_by_id ok': (r) => r.status === 200,
    });
    check(getByVersion(entry.subject), {
      'stress: get_by_version ok': (r) => r.status === 200,
    });
  } else if (roll < 0.85) {
    // 25% writes
    const idx = Math.floor(Math.random() * SCHEMAS.length);
    const base = SCHEMAS[idx];
    const uniqueSubject = `stress-${base.subject}-${__VU}-${__ITER}`;
    check(registerSchema(uniqueSubject, base.schema, base.schemaType), {
      'stress: register ok': (r) => r.status === 200,
    });
  } else {
    // 15% compat checks
    const entry = data.seeded[Math.floor(Math.random() * data.seeded.length)];
    if (entry.schemaType === 'AVRO') {
      const evolved = evolveAvro(
        entry.subject.replace('avro-perf-', 'AvroRec'),
        `stress_${__VU}_${__ITER}`,
      );
      check(testCompatibility(entry.subject, 'latest', evolved), {
        'stress: compat ok': (r) => r.status === 200,
      });
    }
  }

  sleep(0.1);
}
