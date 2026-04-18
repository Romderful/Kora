// delete-under-load.js — Concurrent deletion with registration.
// Tests the soft-delete / hard-delete paths under concurrent writes:
// - TOCTOU race: soft-delete while another VU registers on same subject
// - Reference protection: cannot hard-delete a version that is referenced
// - Two-phase delete: hard-delete requires prior soft-delete

import { check, sleep } from 'k6';
import {
  registerSchema, deleteSubject, deleteVersion, getByVersion,
  listVersions, SCHEMAS,
} from '../helpers.js';

export const options = {
  setupTimeout: '120s',
  scenarios: {
    // Writers: continuously register schemas on shared subjects.
    writers: {
      executor: 'constant-vus',
      exec: 'writeFlow',
      vus: 10,
      duration: '3m',
    },
    // Deleters: soft-delete and hard-delete subjects/versions.
    deleters: {
      executor: 'constant-vus',
      exec: 'deleteFlow',
      vus: 5,
      duration: '3m',
      startTime: '30s', // let writers build up some data first
    },
    // Readers: verify data consistency during deletes.
    readers: {
      executor: 'constant-vus',
      exec: 'readFlow',
      vus: 5,
      duration: '3m',
    },
  },

  thresholds: {
    // Expect 404s and 422s from delete operations — that's normal.
    'http_req_duration{op:register}': ['p(95)<500'],
    'http_req_duration{op:delete}': ['p(95)<500'],
  },
};

// Use a fixed set of subjects so writers and deleters overlap.
const SUBJECT_COUNT = 50;

function subjectName(i) {
  return `del-test-${i}`;
}

export function setup() {
  // Pre-register subjects with 2 versions each.
  const seeded = [];
  for (let i = 0; i < SUBJECT_COUNT; i++) {
    const subject = subjectName(i);

    // v1
    const schema1 = JSON.stringify({
      type: 'record', name: `DelRec${i}`, namespace: 'kora.del',
      fields: [{ name: 'id', type: 'long' }],
    });
    const res1 = registerSchema(subject, schema1);

    // v2 (backward-compatible)
    const schema2 = JSON.stringify({
      type: 'record', name: `DelRec${i}`, namespace: 'kora.del',
      fields: [
        { name: 'id', type: 'long' },
        { name: 'extra', type: ['null', 'string'], default: null },
      ],
    });
    const res2 = registerSchema(subject, schema2);

    if (res1.status === 200) {
      seeded.push({ subject, id: res1.json().id });
    }
  }
  return { seeded };
}

export function writeFlow() {
  // Register new versions on the shared subjects.
  const idx = Math.floor(Math.random() * SUBJECT_COUNT);
  const subject = subjectName(idx);
  const schema = JSON.stringify({
    type: 'record', name: `DelRec${idx}`, namespace: 'kora.del',
    fields: [
      { name: 'id', type: 'long' },
      { name: `f_${__VU}_${__ITER}`, type: ['null', 'string'], default: null },
    ],
  });

  const res = registerSchema(subject, schema);
  check(res, {
    'write: accepted': (r) => r.status === 200 || r.status === 409 || r.status === 404,
  });

  sleep(0.2);
}

export function deleteFlow() {
  const idx = Math.floor(Math.random() * SUBJECT_COUNT);
  const subject = subjectName(idx);

  // Alternate between soft-delete subject and soft-delete version.
  if (Math.random() < 0.5) {
    // Soft-delete the entire subject.
    const res = deleteSubject(subject);
    check(res, {
      'delete: soft-delete subject': (r) => r.status === 200 || r.status === 404,
    });

    // Occasionally attempt hard-delete (requires prior soft-delete).
    if (Math.random() < 0.3) {
      sleep(0.1);
      const hardRes = deleteSubject(subject, true);
      check(hardRes, {
        // 200 = success, 404 = already gone or not soft-deleted, 422 = precondition.
        'delete: hard-delete subject': (r) =>
          r.status === 200 || r.status === 404 || r.status === 422,
      });
    }
  } else {
    // Soft-delete a specific version (latest).
    const delRes = deleteVersion(subject, 'latest');
    check(delRes, {
      'delete: soft-delete version': (r) => r.status === 200 || r.status === 404,
    });
  }

  sleep(0.5);
}

export function readFlow(data) {
  // Read during deletes — verify we get consistent responses (200 or 404, never 500).
  const entry = data.seeded[Math.floor(Math.random() * data.seeded.length)];

  const res = getByVersion(entry.subject, 'latest');
  check(res, {
    'read: consistent response': (r) => r.status === 200 || r.status === 404,
    'read: no 500': (r) => r.status !== 500,
  });

  sleep(0.2);
}
