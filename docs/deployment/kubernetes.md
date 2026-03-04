# Kubernetes Deployment

Deploy TrueFlow on Kubernetes for production workloads with horizontal scaling, health probes, and resource management.

> Helm charts are in development. For now, use the raw manifests below.

---

## Gateway Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: trueflow-gateway
  labels:
    app: trueflow-gateway
spec:
  replicas: 2
  selector:
    matchLabels:
      app: trueflow-gateway
  template:
    metadata:
      labels:
        app: trueflow-gateway
    spec:
      containers:
      - name: gateway
        image: trueflow/gateway:latest
        ports:
        - containerPort: 8443
          name: http
        envFrom:
        - secretRef:
            name: trueflow-secrets
        env:
        - name: TRUEFLOW_ENV
          value: "production"
        - name: RUST_LOG
          value: "info"
        resources:
          requests:
            memory: "256Mi"
            cpu: "500m"
          limits:
            memory: "1Gi"
            cpu: "2"
        livenessProbe:
          httpGet:
            path: /healthz
            port: 8443
          initialDelaySeconds: 10
          periodSeconds: 15
        readinessProbe:
          httpGet:
            path: /readyz
            port: 8443
          initialDelaySeconds: 5
          periodSeconds: 10
```

---

## Service

```yaml
apiVersion: v1
kind: Service
metadata:
  name: trueflow-gateway
spec:
  selector:
    app: trueflow-gateway
  ports:
  - port: 8443
    targetPort: 8443
    protocol: TCP
  type: ClusterIP
```

---

## Secrets

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: trueflow-secrets
type: Opaque
stringData:
  DATABASE_URL: "postgres://user:password@postgres-host:5432/trueflow"
  REDIS_URL: "redis://redis-host:6379"
  TRUEFLOW_MASTER_KEY: "<32-byte-hex-key>"
  TRUEFLOW_ADMIN_KEY: "<strong-random-string>"
  DASHBOARD_SECRET: "<strong-random-string>"
  DASHBOARD_ORIGIN: "https://dashboard.yourdomain.com"
```

> [!CAUTION]
> Never commit secrets to version control. Use Kubernetes Secrets, AWS Secrets Manager, or HashiCorp Vault.

---

## Ingress (Optional)

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: trueflow-gateway
  annotations:
    nginx.ingress.kubernetes.io/ssl-redirect: "true"
spec:
  tls:
  - hosts:
    - gateway.yourdomain.com
    secretName: trueflow-tls
  rules:
  - host: gateway.yourdomain.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: trueflow-gateway
            port:
              number: 8443
```

---

## Health Probes

| Endpoint | Probe Type | What It Checks |
|----------|-----------|----------------|
| `GET /healthz` | Liveness | Process is running and accepting connections |
| `GET /readyz` | Readiness | PostgreSQL and Redis are reachable |
| `GET /metrics` | Monitoring | Prometheus-compatible metrics |
| `GET /health/upstreams` | Monitoring | Circuit breaker state for all upstreams |

---

## Scaling Notes

- The gateway is **stateless** — scale horizontally by increasing `replicas`
- All state lives in PostgreSQL (system of record) and Redis (system of speed)
- Redis is used for rate limiting, caching, and HITL coordination — ensure your Redis instance supports the expected write throughput
- For high availability, use PostgreSQL with read replicas and Redis Sentinel or Cluster mode

---

## Next Steps

- **[Docker](docker.md)** — Development and single-server deployment
- **[Architecture](../reference/architecture.md)** — System design deep dive
- **[Security](../reference/security.md)** — Threat model and encryption details
