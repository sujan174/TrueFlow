#!/bin/bash
set -e

# Run load test suite

usage() {
    echo "Usage: $0 [scenario]"
    echo "Available scenarios:"
    echo "  scenario_1 - Redis Contention"
    echo "  scenario_2 - Connection Soak Test"
    echo "  scenario_3 - CPU-Bound Policy"
    echo "  scenario_4 - Audit Write Saturation"
    echo "  scenario_5 - Circuit Breaker Chaos"
    echo "  scenario_6 - Cache Thrashing"
    echo "  all        - Run all scenarios sequentially"
}

SCENARIO=$1

if [ -z "$SCENARIO" ]; then
    usage
    exit 1
fi

export BASE_URL=${BASE_URL:-"http://localhost:8080"}
export TRUEFLOW_ADMIN_KEY=${TRUEFLOW_ADMIN_KEY:-"tf_admin_dev_key_12345"}

echo "Using BASE_URL: $BASE_URL"

run_scenario() {
    local script=$1
    echo "========================================="
    echo "Running Scenario: $script"
    echo "========================================="
    k6 run "config/$script"
}

case "$SCENARIO" in
    "scenario_1")
        run_scenario "scenario_1_redis.js"
        ;;
    "scenario_2")
        run_scenario "scenario_2_soak.js"
        ;;
    "scenario_3")
        run_scenario "scenario_3_cpu.js"
        ;;
    "scenario_4")
        run_scenario "scenario_4_audit.js"
        ;;
    "scenario_5")
        run_scenario "scenario_5_chaos.js"
        ;;
    "scenario_6")
        run_scenario "scenario_6_cache.js"
        ;;
    "all")
        run_scenario "scenario_1_redis.js"
        sleep 5
        run_scenario "scenario_2_soak.js"
        sleep 5
        run_scenario "scenario_3_cpu.js"
        sleep 5
        run_scenario "scenario_4_audit.js"
        sleep 5
        run_scenario "scenario_5_chaos.js"
        sleep 5
        run_scenario "scenario_6_cache.js"
        ;;
    *)
        echo "Unknown scenario: $SCENARIO"
        usage
        exit 1
        ;;
esac
