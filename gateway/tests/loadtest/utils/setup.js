// setup.js
import http from 'k6/http';
import { check } from 'k6';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';
const ADMIN_BASE_URL = `${BASE_URL}/api`;
const ADMIN_KEY = __ENV.TRUEFLOW_ADMIN_KEY || 'tf_admin_dev_key_12345';
const PROJECT_ID = '00000000-0000-0000-0000-000000000001';

export function setupAdminClient() {
    return {
        headers: {
            'Authorization': `Bearer ${ADMIN_KEY}`,
            'Content-Type': 'application/json'
        }
    };
}

// Creates a policy that allows 999,999 requests/min, overriding the global
// default_rate_limit so load tests aren't capped at the gateway's RPM setting.
export function createRateLimitBypassPolicy(client) {
    const uniqueId = Math.random().toString(36).substring(7);
    const payload = JSON.stringify({
        name: `loadtest-rpm-bypass-${uniqueId}`,
        project_id: PROJECT_ID,
        rules: [{
            when: { '==': [true, true] },
            then: [{ action: 'rate_limit', window: '1m', max_requests: 999999 }]
        }]
    });

    const res = http.post(`${ADMIN_BASE_URL}/v1/policies`, payload, client);
    check(res, { 'rpm-bypass policy created': (r) => r.status === 201 || r.status === 200 });

    if (res.status === 201 || res.status === 200) {
        return JSON.parse(res.body).id;
    }
    console.error(`Failed to create rpm-bypass policy: ${res.status} ${res.body}`);
    return null;
}

export function createTestToken(client, caps = {}, cbConfig = { failure_threshold: 5, reset_timeout_ms: 10000 }, policyIds = []) {
    const uniqueId = Math.random().toString(36).substring(7);
    const body = {
        name: `Load Test Token ${uniqueId}`,
        project_id: PROJECT_ID,
        upstream_url: 'http://localhost:9000',
        circuit_breaker: cbConfig,
    };
    if (policyIds.length > 0) {
        body.policy_ids = policyIds;
    }
    const payload = JSON.stringify(body);

    const res = http.post(`${ADMIN_BASE_URL}/v1/tokens`, payload, client);

    check(res, {
        'token created successfully': (r) => r.status === 201 || r.status === 200,
    });

    if (res.status === 201 || res.status === 200) {
        return JSON.parse(res.body).token_id;
    }
    console.error(`Failed to create token: ${res.status} ${res.body}`);
    return null;
}

export function createComplexPolicy(client) {
    const uniqueId = Math.random().toString(36).substring(7);
    // Correct rule format: when/then (not the flat condition/action/config format)
    const payload = JSON.stringify({
        name: `Complex Load Test Policy ${uniqueId}`,
        project_id: PROJECT_ID,
        rules: [
            {
                when: { '==': [true, true] },
                then: [{
                    action: 'redact',
                    patterns: [
                        '(?i)(api[_-]?key|password|secret)[^a-z0-9]',
                        'sk-[a-zA-Z0-9]{32,}',
                        'xox[baprs]-[0-9]{10,}-[a-zA-Z0-9]{24}'
                    ],
                    replacement: '[REDACTED]'
                }]
            }
        ]
    });

    const res = http.post(`${ADMIN_BASE_URL}/v1/policies`, payload, client);

    check(res, {
        'policy created successfully': (r) => r.status === 201 || r.status === 200,
    });

    if (res.status === 201 || res.status === 200) {
        return JSON.parse(res.body).id;
    }
    console.error(`Failed to create policy: ${res.status} ${res.body}`);
    return null;
}
