import { setupAdminClient, createTestToken } from '../utils/setup.js';
import http from 'k6/http';
import { check, sleep } from 'k6';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';

export const options = {
  scenarios: {
    connection_soak: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '1m', target: 2000 }, // Fast ramp to many connections
        { duration: '5m', target: 5000 }, // Hold high connections for soak
        { duration: '1m', target: 0 },
      ],
      gracefulRampDown: '30s',
    },
  },
};

export function setup() {
    const adminClient = setupAdminClient();
    const tokenId = createTestToken(adminClient);
    return { sharedTokenId: tokenId };
}

export default function (data) {
  const { sharedTokenId } = data;

  const payload = JSON.stringify({
    model: 'mock-model',
    messages: [{ role: 'user', content: 'Write a long essay about the history of Rome.' }],
    stream: true
  });

  const headers = {
    'Authorization': `Bearer ${sharedTokenId}`,
    'Content-Type': 'application/json',
    'x-mock-latency-ms': '2000', // Force slow response
    'x-mock-chunks': '10', // Multiple chunks
  };

  const res = http.post(`${BASE_URL}/v1/chat/completions`, payload, { headers });

  check(res, {
    'is status 200': (r) => r.status === 200,
    'is streaming response': (r) => r.headers['Content-Type'] && r.headers['Content-Type'].includes('text/event-stream'),
  });

  sleep(1);
}
