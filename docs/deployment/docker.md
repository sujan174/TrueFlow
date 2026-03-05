# Docker Deployment

Run the full TrueFlow stack (Gateway + Dashboard + PostgreSQL + Redis) with Docker Compose.

---

## Prerequisites

- [Docker](https://docs.docker.com/get-docker/) and Docker Compose installed
- At least **4 GB RAM** available for Docker (Rust gateway build needs ~3 GB)
- `git` (to clone the repo)

---

## Quick Start

### 1. Clone the Repository

```bash
git clone https://github.com/trueflow/trueflow.git
cd trueflow
```

### 2. Start the Stack

```bash
docker compose up -d --build
```

> First build takes ~2 minutes (Rust gateway compilation + Next.js dashboard build). Subsequent builds use Docker layer caching and take ~10s.

### 3. Access the Dashboard

Open **[http://localhost:3000](http://localhost:3000)**

- **Default Admin Key**: `trueflow-admin-test` (set in `docker-compose.yml`)

---

## What's Running?

| Service | URL / Port | Description |
|---------|-----------|-------------|
| **Dashboard** | `http://localhost:3000` | Web UI for managing tokens, policies, credentials, and audit logs |
| **Gateway** | `http://localhost:8443` | The AI proxy — point your LLM clients here |
| **PostgreSQL** | `localhost:5432` | Database (User: `postgres`, Pass: `password`) |
| **Redis** | `localhost:6379` | Cache, rate limiting, spend counters, HITL queues |

### Optional Services

| Service | Command | URL |
|---------|---------|-----|
| **Jaeger** (Tracing) | `docker compose --profile tracing up -d` | `http://localhost:16686` |
| **Mock Upstream** (Testing) | `docker compose -f docker-compose.yml -f docker-compose.test.yml up -d` | `http://localhost:9000` |

> **Note:** The mock-upstream is defined in a separate `docker-compose.test.yml` overlay — it is not part of the shipping stack.

---

## File Structure

| File | Purpose |
|------|---------|
| `docker-compose.yml` | **Shipping stack:** gateway, dashboard, postgres, redis, jaeger |
| `docker-compose.test.yml` | **Test overlay:** mock-upstream server (for E2E tests) |
| `gateway/Dockerfile` | Multi-stage Rust build (deps cached, ~55s rebuild) |
| `dashboard/Dockerfile` | Multi-stage Next.js build |
| `tests/mock-upstream/Dockerfile` | Python mock LLM server |

> See `docker-compose.yml` for the full configuration. The embedded copy below is omitted to avoid drift — always refer to the actual file.

---

## Configuration

| Variable | What It Does | Default |
|----------|-------------|---------|
| `TRUEFLOW_MASTER_KEY` | 32-byte hex key for vault encryption. **Change for production** | dev key |
| `TRUEFLOW_ADMIN_KEY` | Root admin API key | `trueflow-admin-test` |
| `DATABASE_URL` | PostgreSQL connection string | `postgres://postgres:password@localhost:5432/trueflow` |
| `REDIS_URL` | Redis connection string | `redis://localhost:6379` |
| `DASHBOARD_SECRET` | Dashboard ↔ gateway auth secret | `trueflow-dashboard-dev-secret` |
| `DASHBOARD_ORIGIN` | CORS origin for dashboard | `http://localhost:3000` |
| `TRUEFLOW_ENV` | Set to `production` for secure startup checks | `development` |
| `RUST_LOG` | Log level: `info`, `debug`, `trace` | `info` |
| `TRUEFLOW_PORT` | Gateway bind port | `8443` |
| `TRUEFLOW_SLACK_WEBHOOK_URL` | Slack webhook for HITL notifications | — |
| `TRUEFLOW_ENABLE_TEST_HOOKS` | Enable test headers. **Never in production** | `0` |

---

## Production Checklist

### Secrets

| Variable | What to do |
|----------|-----------|
| `TRUEFLOW_MASTER_KEY` | Generate a cryptographically random 32-byte hex key |
| `TRUEFLOW_ADMIN_KEY` | Set to a strong random string |
| `DASHBOARD_SECRET` | Set to a strong random string |
| `POSTGRES_PASSWORD` | Use a strong password or managed database with IAM auth |

### Infrastructure

- **PostgreSQL 16+** — RDS, Cloud SQL, Supabase, or self-hosted
- **Redis 7+** — ElastiCache, Memorystore, Upstash, or self-hosted
- **TLS** — Terminate at your load balancer (NGINX, Caddy, Traefik)
- **Secrets Management** — K8s Secrets, AWS Secrets Manager, or HashiCorp Vault

---

## Verifying the Installation

```bash
# Check all containers are healthy
docker compose ps

# Gateway health
curl http://localhost:8443/healthz

# Gateway readiness (Postgres + Redis connected)
curl http://localhost:8443/readyz
```

---

## Monitoring

### Health Endpoints

| Endpoint | Purpose |
|----------|---------|
| `GET /healthz` | Liveness — 200 if process is running |
| `GET /readyz` | Readiness — 200 if Postgres and Redis are reachable |
| `GET /metrics` | Prometheus metrics (no auth required) |
| `GET /health/upstreams` | Circuit breaker health for all upstreams |

### Prometheus

Point your Prometheus scrape config at `http://trueflow-gateway:8443/metrics`.

### Recommended Alerts

- Gateway readiness failing (`/readyz` returning non-200)
- Error rate > 5% (`trueflow_requests_total{status=~"5.."}`)
- Latency P99 > 5s (`trueflow_request_duration_seconds`)
- Circuit breaker open (`/health/upstreams` with `is_healthy: false`)

---

## Updating

```bash
git pull
docker compose up -d --build
```

## Stopping

```bash
# Stop (data preserved in Docker volumes)
docker compose down

# Stop and DELETE all data (fresh start)
docker compose down -v
```

---

## Troubleshooting

### "Connection Refused"
Ensure Docker is running. Check `docker compose ps` — all containers should show `healthy`.

### "Gateway container keeps restarting"
Check logs: `docker logs trueflow-gateway-1`. Usually indicates a database connection issue.

### "Dashboard shows Network Error"
The dashboard makes browser-side requests to `NEXT_PUBLIC_API_URL` (default: `http://localhost:8443/api/v1`). Ensure:
1. Gateway container is running and healthy
2. Port 8443 is accessible from your browser
3. `DASHBOARD_ORIGIN` matches the URL you're accessing the dashboard from

### "Build takes too long"
The Rust gateway compilation is CPU-intensive. Ensure at least 2 CPU cores available to Docker.

---

## Next Steps

- **[Quickstart](../getting-started/quickstart.md)** — Create your first credential, policy, and token
- **[Kubernetes](kubernetes.md)** — Deploy on K8s
- **[Python SDK](../sdks/python.md)** / **[TypeScript SDK](../sdks/typescript.md)** — Client libraries
