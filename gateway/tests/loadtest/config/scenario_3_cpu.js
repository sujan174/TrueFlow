import { setupAdminClient, createTestToken, createComplexPolicy, attachPolicyToToken } from '../utils/setup.js';
import http from 'k6/http';
import { check, sleep } from 'k6';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';

export const options = {
  scenarios: {
    cpu_bound_regex: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '30s', target: 200 }, // CPU limits usually hit earlier than IO limits
        { duration: '2m', target: 500 },
        { duration: '30s', target: 0 },
      ],
      gracefulRampDown: '10s',
    },
  },
};

export function setup() {
    const adminClient = setupAdminClient();
    const tokenId = createTestToken(adminClient);
    const policyId = createComplexPolicy(adminClient);
    attachPolicyToToken(adminClient, tokenId, policyId);
    return { sharedTokenId: tokenId };
}

export default function (data) {
  const { sharedTokenId } = data;

  // 100kb payload full of false-positive looking strings to trigger worst-case regex backtracking
  const contentChunk = 'There is a password here maybe: secret, no API_KEY or api-key or sk-12345678901234567890123456789012. My Slack token is not xoxp-1234567890-123456789012345678901234. ';
  const largeContent = contentChunk.repeat(500);

  const payload = JSON.stringify({
    model: 'mock-model',
    messages: [{ role: 'user', content: largeContent }],
  });

  const headers = {
    'Authorization': `Bearer ${sharedTokenId}`,
    'Content-Type': 'application/json',
  };

  const res = http.post(`${BASE_URL}/v1/chat/completions`, payload, { headers });

  check(res, {
    'is status 200': (r) => r.status === 200,
    'latency < 2000ms': (r) => r.timings.duration < 2000,
  });

  sleep(0.5);
}
