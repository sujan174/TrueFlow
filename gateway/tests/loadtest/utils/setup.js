// setup.js
import http from 'k6/http';
import { check } from 'k6';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';
const ADMIN_KEY = __ENV.TRUEFLOW_ADMIN_KEY || 'tf_admin_dev_key_12345';
const PROJECT_ID = 'loadtest_project';

export function setupAdminClient() {
    return {
        headers: {
            'Authorization': `Bearer ${ADMIN_KEY}`,
            'Content-Type': 'application/json'
        }
    };
}

export function createTestToken(client, caps = {}, cbConfig = { failure_threshold: 5, reset_timeout_ms: 10000 }) {
    const payload = JSON.stringify({
        name: `Load Test Token ${__VU}-${__ITER}`,
        project_id: PROJECT_ID,
        provider: 'mock',
        spend_caps: caps,
        circuit_breaker: cbConfig
    });

    const res = http.post(`${BASE_URL}/v1/admin/tokens`, payload, client);

    check(res, {
        'token created successfully': (r) => r.status === 201 || r.status === 200,
    });

    if (res.status === 201 || res.status === 200) {
        return JSON.parse(res.body).id;
    }
    console.error(`Failed to create token: ${res.status} ${res.body}`);
    return null;
}

export function createComplexPolicy(client) {
    const payload = JSON.stringify({
        name: `Complex Load Test Policy ${__VU}-${__ITER}`,
        project_id: PROJECT_ID,
        version: "1.0",
        rules: [
            {
                name: "redact_secrets",
                phase: "pre",
                mode: "enforce",
                condition: { "==": [true, true] },
                action: "redact",
                config: {
                    patterns: [
                        "(?i)(api[_-]?key|password|secret)[^a-z0-9]",
                        "sk-[a-zA-Z0-9]{32,}",
                        "xox[baprs]-[0-9]{10,}-[a-zA-Z0-9]{24}"
                    ],
                    replacement: "[REDACTED]"
                }
            }
        ]
    });

    const res = http.post(`${BASE_URL}/v1/admin/policies`, payload, client);

    check(res, {
        'policy created successfully': (r) => r.status === 201 || r.status === 200,
    });

    if (res.status === 201 || res.status === 200) {
        return JSON.parse(res.body).id;
    }
    console.error(`Failed to create policy: ${res.status} ${res.body}`);
    return null;
}

export function attachPolicyToToken(client, tokenId, policyId) {
    const payload = JSON.stringify({
        token_id: tokenId,
        policy_id: policyId,
        priority: 10
    });

    const res = http.post(`${BASE_URL}/v1/admin/tokens/${tokenId}/policies`, payload, client);

    check(res, {
        'policy attached successfully': (r) => r.status === 201 || r.status === 200,
    });
}
