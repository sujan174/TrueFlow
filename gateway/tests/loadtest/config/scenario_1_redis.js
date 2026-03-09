import { setupAdminClient, createTestToken, createRateLimitBypassPolicy } from '../utils/setup.js';
import http from 'k6/http';
import { check, sleep } from 'k6';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';

export const options = {
  scenarios: {
    redis_contention: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '30s', target: 500 },
        { duration: '30s', target: 1000 },
        { duration: '1m', target: 1000 }, // Hold peak
        { duration: '30s', target: 0 },
      ],
      gracefulRampDown: '10s',
    },
  },
};

export function setup() {
    // Generate a SINGLE token for all VUs to hit simultaneously to test Redis lock contention
    const adminClient = setupAdminClient();
    const bypassPolicyId = createRateLimitBypassPolicy(adminClient);
    const tokenId = createTestToken(adminClient, {}, undefined, bypassPolicyId ? [bypassPolicyId] : []);
    return { sharedTokenId: tokenId };
}

export default function (data) {
  const { sharedTokenId } = data;

  const payload = JSON.stringify({
    model: 'mock-model-cheap',
    messages: [{ role: 'user', content: 'What is the capital of France?' }],
  });

  const headers = {
    'Authorization': `Bearer ${sharedTokenId}`,
    'Content-Type': 'application/json',
  };

  const res = http.post(`${BASE_URL}/v1/chat/completions`, payload, { headers });

  check(res, {
    'is status 200 or 402': (r) => r.status === 200 || r.status === 402, // 402 is expected once caps hit
    'no 500 errors': (r) => r.status !== 500,
  });

  // Short sleep to prevent immediate retry thrashing, simulating real bursty traffic
  sleep(Math.random() * 0.1);
}
