# API Endpoint Alignment Verification Report

## Summary

**Date**: 2026-03-21
**Frontend**: `dashboard/src/lib/api.ts`
**Backend**: `gateway/src/api/mod.rs` + `gateway/src/api/handlers/analytics.rs`

### Verification Table

| Tab | Frontend Function | Frontend Endpoint | Backend Route | Status |
|-----|-------------------|-------------------|---------------|--------|
| Overview | `getAnalyticsSummary()` | `/analytics/summary?range=${hours}` | `/analytics/summary` | ✅ ALIGNED |
| Overview | `getModelUsage()` | `/analytics/models?range=${hours}` | `/analytics/models` | ✅ ALIGNED |
| Traffic | `getTrafficTimeseries()` | `/analytics/traffic/timeseries?range=${hours}` | **MISSING** | ❌ MISMATCH |
| Cost | `getBudgetHealth()` | `/analytics/budget-health` | `/analytics/budget-health` | ✅ ALIGNED |
| Cost | `getSpendTimeseries()` | `/analytics/spend/timeseries?group_by=${groupBy}&range=${hours}` | **MISSING** | ❌ MISMATCH |
| Users | `getUserGrowth()` | `/analytics/users/growth?range=${hours}` | `/analytics/users/growth` | ✅ ALIGNED |
| Users | `getTokenAlerts()` | `/analytics/tokens/alerts?range=${hours}` | `/analytics/tokens/alerts` | ✅ ALIGNED |
| Cache | `getCacheSummary()` | `/analytics/cache/summary?range=${hours}` | `/analytics/cache/summary` | ✅ ALIGNED |
| Models | `getModelStats()` | `/analytics/models/stats?range=${hours}` | **MISSING** | ❌ MISMATCH |
| Models | `getCostLatencyScatter()` | `/analytics/models/cost-latency-scatter?range=${hours}` | **MISSING** | ❌ MISMATCH |
| Security | `getSecuritySummary()` | `/analytics/security/summary?range=${hours}` | `/analytics/security/summary` | ✅ ALIGNED |
| Security | `getShadowPolicies()` | `/analytics/security/shadow-policies?range=${hours}` | `/analytics/security/shadow-policies` | ✅ ALIGNED |
| HITL | `getHitlSummary()` | `/analytics/hitl/summary?range=${hours}` | `/analytics/hitl/summary` | ✅ ALIGNED |
| HITL | `listApprovals()` | `/approvals` | **MISSING** | ❌ MISMATCH |
| Errors | `getErrorSummary()` | `/analytics/errors/summary?range=${hours}` | `/analytics/errors/summary` | ✅ ALIGNED |
| Errors | `getErrorLogs()` | `/analytics/errors/logs?limit=${limit}` | `/analytics/errors/logs` | ✅ ALIGNED |

---

## Issues Found

### 1. Missing Backend Routes (5 endpoints)

These frontend functions call endpoints that do not exist in the backend:

| Frontend Function | Missing Backend Route |
|-------------------|----------------------|
| `getTrafficTimeseries()` | `/analytics/traffic/timeseries` |
| `getSpendTimeseries()` | `/analytics/spend/timeseries` |
| `getModelStats()` | `/analytics/models/stats` |
| `getCostLatencyScatter()` | `/analytics/models/cost-latency-scatter` |
| `listApprovals()` | `/approvals` |

### 2. Route Exists But Not in Expected Location

| Route | Current Path | Expected |
|-------|--------------|----------|
| Approvals decision | `/approvals/:id/decision` (POST) | Also needs `/approvals` (GET) |

### 3. Handler Analysis

**Handlers that exist in `analytics.rs` but routes NOT registered in `mod.rs`:**

The following handlers were found in `analytics.rs`:
- `get_traffic_timeseries` - handler exists, route missing
- `get_spend_timeseries` - handler exists, route missing
- `get_model_stats` - handler exists, route missing
- `get_cost_latency_scatter` - handler exists, route missing

**Note**: The handlers are implemented but the routes are NOT registered in `api/mod.rs`.

---

## Detailed Route Comparison

### Backend Routes in `mod.rs` (Analytics-related):

```
/analytics/tokens
/analytics/volume
/analytics/status
/analytics/models
/analytics/summary
/analytics/users
/analytics/budget-health
/analytics/burn-rate
/analytics/token-spend
/analytics/users/growth
/analytics/tokens/alerts
/analytics/cache/summary
/analytics/hitl/summary
/analytics/hitl/volume
/analytics/hitl/latency
/analytics/errors/summary
/analytics/errors/timeseries
/analytics/errors/breakdown
/analytics/errors/logs
```

### Frontend Endpoints (Analytics-related):

```
/analytics/summary
/analytics/models
/analytics/traffic/timeseries      <-- MISSING
/analytics/budget-health
/analytics/spend/timeseries        <-- MISSING
/analytics/users/growth
/analytics/tokens/alerts
/analytics/cache/summary
/analytics/models/stats            <-- MISSING
/analytics/models/cost-latency-scatter  <-- MISSING
/analytics/security/summary
/analytics/security/shadow-policies
/analytics/hitl/summary
/analytics/errors/summary
/analytics/errors/logs
/approvals                         <-- MISSING
```

---

## Recommendations

### Critical (Must Fix)

1. **Add missing route registrations in `gateway/src/api/mod.rs`**:

```rust
// Traffic analytics
.route("/analytics/traffic/timeseries", get(handlers::get_traffic_timeseries))

// Cost analytics
.route("/analytics/spend/timeseries", get(handlers::get_spend_timeseries))

// Model analytics
.route("/analytics/models/stats", get(handlers::get_model_stats))
.route("/analytics/models/cost-latency-scatter", get(handlers::get_cost_latency_scatter))

// HITL approvals
.route("/approvals", get(handlers::list_approvals))
```

2. **Implement `list_approvals` handler** if not already present in handlers module.

### Verification Steps

After fixing:
1. Run `cargo test` to verify no compilation errors
2. Test each endpoint with curl or Postman
3. Verify frontend dashboard loads all tabs without errors