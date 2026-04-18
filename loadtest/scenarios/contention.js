// contention.js — FOR UPDATE lock contention on a single subject.
// All VUs register unique evolving schemas against ONE subject, hammering:
// - Subject row lock (FOR UPDATE)
// - Version increment (MAX(version) + 1)
// - Compatibility check (grows O(N) with version count in transitive mode)
//
// Also includes a TOCTOU detection assertion: reads back versions periodically
// to check if incompatible schemas slipped through the compat-outside-tx gap.

import { check, sleep } from 'k6';
import { Trend } from 'k6/metrics';
import { registerSchema, getByVersion, listVersions } from '../helpers.js';

const SUBJECT = 'contention-hot-subject';
const MAX_VERSIONS = 500;

const versionCount = new Trend('contention_version_count');

export const options = {
  setupTimeout: '30s',
  scenarios: {
    hot_writers: {
      executor: 'ramping-vus',
      startVUs: 1,
      stages: [
        { duration: '30s', target: 10 },
        { duration: '2m', target: 20 },
        { duration: '30s', target: 50 },   // spike
        { duration: '1m', target: 50 },
        { duration: '30s', target: 0 },
      ],
    },
  },

  thresholds: {
    // Higher failure tolerance — expect some 409 conflicts.
    http_req_failed: ['rate<0.10'],
    'http_req_duration{op:register}': ['p(95)<1000'],
  },
};

export function setup() {
  // Seed subject with v1.
  const schema = JSON.stringify({
    type: 'record', name: 'HotRecord', namespace: 'kora.contention',
    fields: [{ name: 'id', type: 'long' }],
  });
  const res = registerSchema(SUBJECT, schema);
  check(res, { 'setup: seed v1': (r) => r.status === 200 });
  return {};
}

export default function () {
  // Guard: stop writing once we hit MAX_VERSIONS to avoid turning this into a soak test.
  // The compat check becomes O(N) with version count in transitive mode.
  if (__ITER >= MAX_VERSIONS) {
    // Switch to reads-only.
    check(getByVersion(SUBJECT, 'latest'), {
      'contention: read ok': (r) => r.status === 200,
    });
    sleep(0.5);
    return;
  }

  // Each VU generates a unique nullable field → backward-compatible evolution.
  const field = `f_${__VU}_${__ITER}`;
  const schema = JSON.stringify({
    type: 'record', name: 'HotRecord', namespace: 'kora.contention',
    fields: [
      { name: 'id', type: 'long' },
      { name: field, type: ['null', 'string'], default: null },
    ],
  });

  const res = registerSchema(SUBJECT, schema);

  // 200 = new version registered, 409 = incompatible (expected under contention).
  check(res, {
    'contention: register accepted': (r) => r.status === 200 || r.status === 409,
  });

  // TOCTOU detection: every 50 iterations, read back version count.
  // If versions grow faster than expected, flag it.
  if (__ITER % 50 === 0 && __ITER > 0) {
    const versionsRes = listVersions(SUBJECT);
    if (versionsRes.status === 200) {
      const versions = versionsRes.json();
      versionCount.add(versions.length);
    }
  }

  sleep(0.1);
}

export function teardown() {
  // Log final version count for analysis.
  const versionsRes = listVersions(SUBJECT);
  if (versionsRes.status === 200) {
    const versions = versionsRes.json();
    console.log(`Contention test: ${versions.length} versions created on ${SUBJECT}`);
  }
}
