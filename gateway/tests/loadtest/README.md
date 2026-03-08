# TrueFlow Pre-Launch Load Testing Suite

This directory contains `k6` load testing scripts to validate TrueFlow's performance under high load before launch.

## Setup
1. Ensure `k6` is installed.
2. The TrueFlow Gateway should be running locally or in Docker.
3. A mock upstream server should be running if using `mock` provider tokens.

## Running Tests
Use the provided `run.sh` script or npm scripts to execute tests.

```bash
# Run a specific scenario
./run.sh scenario_1

# Run all scenarios sequentially
./run.sh all
```

Or using `npm`:
```bash
npm run test:1
npm run test:all
```

## Scenarios
- `scenario_1_redis.js` - Redis Contention
- `scenario_2_soak.js` - Connection Exhaustion (Soak Test)
- `scenario_3_cpu.js` - CPU-Bound Policy Stress
- `scenario_4_audit.js` - Audit Write Saturation
- `scenario_5_chaos.js` - Circuit Breaker Chaos
- `scenario_6_cache.js` - Cache Thrashing
