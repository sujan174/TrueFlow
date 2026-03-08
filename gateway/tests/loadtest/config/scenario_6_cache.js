import { setupAdminClient, createTestToken } from '../utils/setup.js';
import http from 'k6/http';
import { check, sleep } from 'k6';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';

export const options = {
  scenarios: {
    cache_thrashing: {
      executor: 'constant-vus',
      vus: 200,
      duration: '2m',
    },
  },
};

export function setup() {
    console.log("Setting up thousands of tokens to bypass L1/L2 caches... This takes a moment.");
    const adminClient = setupAdminClient();
    const tokenIds = [];

    // Create 500 unique tokens to guarantee cache eviction (assuming L1 size is smaller)
    for(let i=0; i<500; i++) {
        const id = createTestToken(adminClient);
        if (id) {
            tokenIds.push(id);
        }
    }
    console.log(`Created ${tokenIds.length} tokens for cache thrashing.`);
    return { tokenIds };
}

export default function (data) {
  const { tokenIds } = data;

  // Randomly select a token every request to guarantee cache misses
  const tokenId = tokenIds[Math.floor(Math.random() * tokenIds.length)];

  const payload = JSON.stringify({
    model: 'mock-model',
    messages: [{ role: 'user', content: 'Random Token Request' }],
  });

  const headers = {
    'Authorization': `Bearer ${tokenId}`,
    'Content-Type': 'application/json',
  };

  const res = http.post(`${BASE_URL}/v1/chat/completions`, payload, { headers });

  check(res, {
    'status 200': (r) => r.status === 200,
  });

  // Small sleep to maintain steady DB load without entirely overwhelming PG connections
  sleep(0.1);
}
