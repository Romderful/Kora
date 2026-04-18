// soak.js — Long-running accumulation test.
// 30 VUs for 2 hours (configurable via K6_SOAK_DURATION env var).
// Observes: query degradation as tables grow, dead tuple accumulation,
// index efficiency, memory growth, pool behavior over time.
// Use --out csv=soak-results.csv to avoid JSON output bloat.

import { check, sleep } from 'k6';
import {
  seedSchemas, getById, getByVersion, listSubjects,
  registerSchema, testCompatibility, evolveAvro, scrapeMetrics,
  SCHEMAS,
} from '../helpers.js';

const SOAK_DURATION = __ENV.K6_SOAK_DURATION || '2h';

export const options = {
  setupTimeout: '120s',
  scenarios: {
    soak_mix: {
      executor: 'constant-vus',
      exec: 'soakMix',
      vus: 30,
      duration: SOAK_DURATION,
    },
    prom_scrape: {
      executor: 'constant-arrival-rate',
      exec: 'scrapeFlow',
      rate: 1,
      timeUnit: '10s',
      duration: SOAK_DURATION,
      preAllocatedVUs: 1,
    },
  },

  thresholds: {
    http_req_failed: ['rate<0.01'],
    'http_req_duration{op:get_by_id}': ['p(95)<100'],
    'http_req_duration{op:register}': ['p(95)<500'],
  },
};

export function setup() {
  // Smaller seed — soak accumulates data during the run.
  const seeded = seedSchemas(200);
  return { seeded };
}

export function soakMix(data) {
  const roll = Math.random();

  if (roll < 0.50) {
    // 50% reads (observe degradation as data grows)
    const entry = data.seeded[Math.floor(Math.random() * data.seeded.length)];
    check(getById(entry.id), { 'soak: read ok': (r) => r.status === 200 });
    check(getByVersion(entry.subject), { 'soak: version ok': (r) => r.status === 200 });

    if (Math.random() < 0.1) {
      // Use prefix filter — realistic pattern (no client lists all 100K+ subjects).
      check(listSubjects('avro-perf-1'), { 'soak: list ok': (r) => r.status === 200 });
    }

  } else if (roll < 0.85) {
    // 35% writes (accumulate data — the whole point of soak)
    // Mix all 3 formats to exercise different diff engines.
    const idx = Math.floor(Math.random() * SCHEMAS.length);
    const base = SCHEMAS[idx];
    const uniqueSubject = `soak-${base.subject}-${__VU}-${__ITER}`;

    check(registerSchema(uniqueSubject, base.schema, base.schemaType), {
      'soak: register ok': (r) => r.status === 200,
    });

  } else {
    // 15% compat checks (observe CPU impact as versions grow)
    const entry = data.seeded[Math.floor(Math.random() * data.seeded.length)];
    if (entry.schemaType === 'AVRO') {
      const evolved = evolveAvro(
        entry.subject.replace('avro-perf-', 'AvroRec'),
        `soak_${__VU}_${__ITER}`,
      );
      check(testCompatibility(entry.subject, 'latest', evolved), {
        'soak: compat ok': (r) => r.status === 200,
      });
    }
  }

  sleep(0.3);
}

export function scrapeFlow() {
  scrapeMetrics();
}
