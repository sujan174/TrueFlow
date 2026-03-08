import { setupAdminClient, createTestToken } from '../utils/setup.js';
import http from 'k6/http';
import { check, sleep } from 'k6';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';

export const options = {
  scenarios: {
    audit_write_saturation: {
      executor: 'constant-arrival-rate',
      rate: 1000, // 1000 requests per second
      timeUnit: '1s',
      duration: '3m',
      preAllocatedVUs: 500,
      maxVUs: 2000,
    },
  },
};

export function setup() {
    const adminClient = setupAdminClient();

    // We create multiple tokens to spread out L1 cache hits and force more Postgres interactions if needed,
    // though the main goal here is Audit Store INSERTS
    const tokenIds = [];
    for(let i=0; i<10; i++) {
        tokenIds.push(createTestToken(adminClient));
    }
    return { tokenIds };
}

export default function (data) {
  const { tokenIds } = data;
  const tokenId = tokenIds[Math.floor(Math.random() * tokenIds.length)];

  const payload = JSON.stringify({
    model: 'mock-model',
    messages: [{ role: 'user', content: 'Generate a short poem.' }],
  });

  const headers = {
    'Authorization': `Bearer ${tokenId}`,
    'Content-Type': 'application/json',
    'x-mock-latency-ms': '10', // Fast mock responses to saturate audit writes quickly
  };

  const res = http.post(`${BASE_URL}/v1/chat/completions`, payload, { headers });

  check(res, {
    'is status 200': (r) => r.status === 200,
  });
}
