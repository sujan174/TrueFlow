# Experiments and A/B Testing

TrueFlow provides built-in support for A/B testing and traffic splitting between models, prompts, and routing strategies. Experiments are a convenience layer over the policy engine's `Split` action.

---

## Overview

Experiments enable you to:

- Compare model performance (GPT-4 vs Claude)
- Test prompt variations
- Roll out changes gradually (canary deployments)
- Measure cost, latency, and error rates across variants

---

## Quick Start

### Create an Experiment

```bash
curl -X POST http://localhost:8443/api/v1/experiments \
  -H "Authorization: Bearer $ADMIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "gpt4o-vs-claude",
    "variants": [
      { "name": "control", "weight": 50, "model": "gpt-4o" },
      { "name": "treatment", "weight": 50, "model": "claude-3-5-sonnet-20241022" }
    ]
  }'
```

This creates an experiment that routes 50% of traffic to GPT-4o and 50% to Claude 3.5 Sonnet.

### Monitor Results

```bash
curl "http://localhost:8443/api/v1/experiments/{id}/results" \
  -H "Authorization: Bearer $ADMIN_KEY"
```

Response:

```json
{
  "experiment": "gpt4o-vs-claude",
  "status": "running",
  "variants": [
    {
      "variant": "control",
      "total_requests": 1240,
      "avg_latency_ms": 342,
      "total_cost_usd": 12.45,
      "error_rate": 0.01
    },
    {
      "variant": "treatment",
      "total_requests": 1238,
      "avg_latency_ms": 289,
      "total_cost_usd": 8.72,
      "error_rate": 0.00
    }
  ]
}
```

### Stop an Experiment

```bash
curl -X POST "http://localhost:8443/api/v1/experiments/{id}/stop" \
  -H "Authorization: Bearer $ADMIN_KEY"
```

---

## How Experiments Work

### Architecture

When you create an experiment, TrueFlow:

1. Creates a `Split` policy action with the specified variants
2. Attaches the policy to the token(s) you specify
3. Tracks metrics per variant in the audit logs
4. Provides analytics endpoints for analysis

### Variant Selection

Variant selection is **deterministic per request**:

- Selection is based on `request_id` (unique per request)
- The same caller gets the same variant within a single request
- This prevents users from seeing inconsistent behavior

### Weight Distribution

Weights are **relative**, not absolute:

- Weights `50, 50` = 50/50 split
- Weights `70, 30` = 70/30 split
- Weights `1, 1` = 50/50 split
- Weights `3, 1` = 75/25 split

---

## API Reference

### Create Experiment

`POST /experiments`

```json
{
  "name": "experiment-name",
  "token_id": "tf_v1_...",           // Optional: attach to specific token
  "variants": [
    {
      "name": "control",
      "weight": 50,
      "model": "gpt-4o",
      "upstream_url": "https://api.openai.com"  // Optional override
    },
    {
      "name": "treatment",
      "weight": 50,
      "model": "claude-3-5-sonnet-20241022",
      "upstream_url": "https://api.anthropic.com"
    }
  ]
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Experiment identifier |
| `token_id` | string | No | Token to attach experiment to |
| `variants` | array | Yes | List of variants with weights |
| `variant.name` | string | Yes | Variant identifier |
| `variant.weight` | integer | Yes | Relative weight for traffic distribution |
| `variant.model` | string | Yes | Model to use for this variant |
| `variant.upstream_url` | string | No | Override upstream URL |

### List Experiments

`GET /experiments`

Returns all running experiments.

### Get Experiment

`GET /experiments/{id}`

Returns experiment configuration and current status.

### Get Results

`GET /experiments/{id}/results`

Returns aggregated metrics per variant:

| Metric | Description |
|--------|-------------|
| `total_requests` | Number of requests routed to this variant |
| `avg_latency_ms` | Average response latency |
| `total_cost_usd` | Total spend for this variant |
| `error_rate` | Fraction of requests that returned errors |

### Update Weights

`PUT /experiments/{id}`

```json
{
  "variants": [
    { "name": "control", "weight": 30 },
    { "name": "treatment", "weight": 70 }
  ]
}
```

Adjust variant weights mid-experiment without stopping.

### Stop Experiment

`POST /experiments/{id}/stop`

Soft-deletes the underlying Split policy and marks the experiment as stopped.

---

## Use Cases

### Model Comparison

Compare two models on cost and latency:

```json
{
  "name": "model-comparison",
  "variants": [
    { "name": "gpt4o", "weight": 50, "model": "gpt-4o" },
    { "name": "haiku", "weight": 50, "model": "claude-3-haiku-20240307" }
  ]
}
```

### Canary Deployment

Gradually roll out a new model:

```json
{
  "name": "canary-claude",
  "variants": [
    { "name": "stable", "weight": 95, "model": "gpt-4o-mini" },
    { "name": "canary", "weight": 5, "model": "claude-3-5-sonnet-20241022" }
  ]
}
```

Then increase canary weight as confidence grows:

```bash
# Shift to 80/20
curl -X PUT http://localhost:8443/api/v1/experiments/{id} \
  -d '{"variants": [{"name": "stable", "weight": 80}, {"name": "canary", "weight": 20}]}'

# Shift to 50/50
curl -X PUT http://localhost:8443/api/v1/experiments/{id} \
  -d '{"variants": [{"name": "stable", "weight": 50}, {"name": "canary", "weight": 50}]}'
```

### Cost Optimization

Find the cheapest model that meets quality requirements:

```json
{
  "name": "cost-optimization",
  "variants": [
    { "name": "gpt4o-mini", "weight": 33, "model": "gpt-4o-mini" },
    { "name": "haiku", "weight": 33, "model": "claude-3-haiku-20240307" },
    { "name": "flash", "weight": 34, "model": "gemini-2.0-flash" }
  ]
}
```

### Provider Failover

Test fallback behavior:

```json
{
  "name": "failover-test",
  "variants": [
    {
      "name": "primary",
      "weight": 90,
      "model": "gpt-4o",
      "upstream_url": "https://api.openai.com"
    },
    {
      "name": "backup",
      "weight": 10,
      "model": "gpt-4o",
      "upstream_url": "https://openai-backup.example.com"
    }
  ]
}
```

---

## Advanced: Manual Split Policies

For more control, create a `Split` policy action directly:

```json
{
  "name": "custom-traffic-split",
  "rules": [
    {
      "when": { "always": true },
      "then": {
        "action": "split",
        "experiment": "my-custom-experiment",
        "variants": [
          {
            "weight": 70,
            "name": "control",
            "set_body_fields": {"model": "gpt-4o", "temperature": 0.7}
          },
          {
            "weight": 30,
            "name": "experiment",
            "set_body_fields": {"model": "claude-3-5-sonnet-20241022", "temperature": 0.5}
          }
        ]
      }
    }
  ]
}
```

This allows you to:

- Modify multiple body fields (model + temperature)
- Add conditions (only split certain requests)
- Combine with other actions

---

## Metrics and Analysis

### Query Audit Logs

Filter by experiment name:

```bash
curl "http://localhost:8443/api/v1/audit?experiment=gpt4o-vs-claude" \
  -H "Authorization: Bearer $ADMIN_KEY"
```

### Analytics Endpoints

```bash
# Experiment-specific analytics
curl "http://localhost:8443/api/v1/analytics/experiments" \
  -H "Authorization: Bearer $ADMIN_KEY"
```

### Custom Analysis

Export audit logs to your analytics platform:

```bash
curl "http://localhost:8443/api/v1/audit/stream" \
  -H "Authorization: Bearer $ADMIN_KEY" | \
  jq 'select(.experiment_name == "gpt4o-vs-claude")'
```

---

## Best Practices

### 1. Start Small

Begin with a small canary percentage (1-5%) and increase gradually.

### 2. Monitor Key Metrics

- **Latency**: Is the new model faster?
- **Cost**: Is the new model cheaper?
- **Error Rate**: Is the new model reliable?
- **Quality**: Subjective assessment of outputs

### 3. Use Meaningful Names

```json
{
  "name": "2026-q1-sonnet-migration",  // Good: descriptive
  "variants": [
    { "name": "baseline-gpt4o", ... },  // Good: clear purpose
    { "name": "candidate-sonnet", ... }
  ]
}
```

### 4. Clean Up

Stop experiments when complete:

```bash
curl -X POST http://localhost:8443/api/v1/experiments/{id}/stop
```

### 5. Document Results

Record experiment outcomes for future reference:

- Which variant won?
- What metrics drove the decision?
- What date was the switch made?