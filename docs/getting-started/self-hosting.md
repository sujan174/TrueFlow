# Self-Hosting TrueFlow

Run the full TrueFlow stack (Gateway + Dashboard + PostgreSQL + Redis) on your machine or server.

## Prerequisites

- [Docker](https://docs.docker.com/get-docker/) and Docker Compose installed
- At least **4 GB RAM** available for the stack
- `git` (to clone the repo)

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

> This may take 5–10 minutes the first time as it compiles the Rust gateway and builds the Next.js dashboard.

### 3. Access the Dashboard

Open your browser → **[http://localhost:3000](http://localhost:3000)**

- **Default Admin Key**: `trueflow-admin-test` (set in `docker-compose.yml`)

## What's Running?

| Service | URL / Port | Description |
|---------|-----------|-------------|
| **Dashboard** | `http://localhost:3000` | Web UI for managing tokens, policies, credentials, and audit logs |
| **Gateway** | `http://localhost:8443` | The AI proxy. Point your LLM clients and agents here |
| **PostgreSQL** | `localhost:5432` | Database (User: `postgres`, Pass: `password`) |
| **Redis** | `localhost:6379` | Cache, rate limiting, spend counters, HITL queues |

### Optional Services

| Service | Command to Enable | URL |
|---------|-------------------|-----|
| **Jaeger** (Tracing) | `docker compose --profile tracing up -d` | `http://localhost:16686` |
| **Mock Upstream** (Testing) | `docker compose up mock-upstream -d` | `http://localhost:9000` |

## Configuration

Edit `docker-compose.yml` to customize your deployment:

| Variable | What It Does | Default |
|----------|-------------|---------|
| `TRUEFLOW_MASTER_KEY` | Encryption key for the credential vault. **Change for production** | dev key |
| `TRUEFLOW_ADMIN_KEY` | Root admin API key for the Management API | `trueflow-admin-test` |
| `DASHBOARD_SECRET` | Shared secret for dashboard ↔ gateway auth | `trueflow-dashboard-dev-secret` |
| `DASHBOARD_ORIGIN` | CORS origin for the dashboard | `http://localhost:3000` |
| Port `8443` | Gateway port (change `"8443:8443"` if conflicts) | `8443` |
| Port `3000` | Dashboard port (change `"3000:3000"` if conflicts) | `3000` |

## Verifying the Installation

```bash
# Check all containers are healthy
docker compose ps

# Check gateway health
curl http://localhost:8443/healthz

# Check gateway readiness (Postgres + Redis connected)
curl http://localhost:8443/readyz
```

## Updating

```bash
git pull
docker compose up -d --build
```

## Stopping

```bash
# Stop all services (data is preserved in Docker volumes)
docker compose down

# Stop and DELETE all data (start fresh)
docker compose down -v
```

## Troubleshooting

### "Connection Refused"
Ensure Docker is running. Check `docker compose ps` — all containers should show `healthy`.

### "Gateway container keeps restarting"
Check logs: `docker logs trueflow-gateway-1`. Usually indicates a database connection issue — ensure PostgreSQL is healthy first.

### "Dashboard shows Network Error"
The dashboard makes browser-side requests to `NEXT_PUBLIC_API_URL` (default: `http://localhost:8443/api/v1`). Ensure:
1. The gateway container is running and healthy
2. Port 8443 is accessible from your browser
3. `DASHBOARD_ORIGIN` matches the URL you're accessing the dashboard from

### "Build takes too long"
The Rust gateway compilation is CPU-intensive. On the first build:
- Ensure at least 2 CPU cores available to Docker
- Subsequent builds use Docker layer caching and are much faster

## Next Steps

- **[Quickstart Guide](../getting-started/quickstart.md)** — Create your first credential, policy, and token
- **[Deployment Guide](../deployment/kubernetes.md)** — Production deployment with Kubernetes
- **[Python SDK](../sdks/python.md)** / **[TypeScript SDK](../sdks/typescript.md)** — Client libraries
