import { setupAdminClient, createTestToken } from '../utils/setup.js';
import http from 'k6/http';
import { check, sleep } from 'k6';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';

export const options = {
  scenarios: {
    chaos_circuit_breaker: {
      executor: 'ramping-arrival-rate',
      startRate: 50,
      timeUnit: '1s',
      preAllocatedVUs: 100,
      maxVUs: 1000,
      stages: [
        { target: 200, duration: '30s' }, // Ramp up baseline traffic
        { target: 500, duration: '1m' }, // Inject chaos here (simulated by the header on 50% of requests)
        { target: 50, duration: '30s' }, // Cool down, should see quick recovery when chaos stops
      ],
    },
  },
};

export function setup() {
    const adminClient = setupAdminClient();
    const tokenId = createTestToken(adminClient, {}, { failure_threshold: 3, reset_timeout_ms: 10000 });
    return { sharedTokenId: tokenId };
}

export default function (data) {
  const { sharedTokenId } = data;

  const injectChaos = Math.random() < 0.5; // 50% chance to force upstream 500

  const payload = JSON.stringify({
    model: 'mock-model',
    messages: [{ role: 'user', content: 'Chaos!' }],
  });

  const headers = {
    'Authorization': `Bearer ${sharedTokenId}`,
    'Content-Type': 'application/json',
  };

  if (injectChaos) {
      headers['x-mock-flaky'] = 'true'; // Tell mock-upstream to fail
  }

  const res = http.post(`${BASE_URL}/v1/chat/completions`, payload, { headers });

  // With a 50% error rate and threshold=3, circuit will trip constantly.
  // We expect either 200 (success), 500 (mock upstream failed before trip), or 503 (Circuit Breaker OPEN).
  check(res, {
    'expected response types': (r) => r.status === 200 || r.status === 500 || r.status === 503,
    'circuit breaker active': (r) => r.status === 503, // If this never hits, CB isn't working
  });

  sleep(0.1);
}
