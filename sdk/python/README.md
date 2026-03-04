# TrueFlow Python SDK

Python client for the [TrueFlow Gateway](https://github.com/sujan174/trueflow) — secure credential management and policy enforcement for AI agents.

## Installation

```bash
pip install trueflow
```

With provider extras:

```bash
pip install trueflow[openai]      # OpenAI compatibility
pip install trueflow[anthropic]   # Anthropic compatibility
```

With framework integrations:

```bash
pip install trueflow[langchain]    # LangChain support
pip install trueflow[crewai]       # CrewAI support
pip install trueflow[llamaindex]   # LlamaIndex support
pip install trueflow[frameworks]   # All frameworks
```

## Framework Integrations

TrueFlow integrates natively with LangChain, CrewAI, and LlamaIndex:

```python
from trueflow import TrueFlowClient
from trueflow.integrations import langchain_chat, crewai_llm, llamaindex_llm

client = TrueFlowClient()

# LangChain
llm = langchain_chat(client, model="gpt-4o")
chain = prompt | llm

# CrewAI
llm = crewai_llm(client, model="gpt-4o")
agent = Agent(role="Researcher", llm=llm, ...)

# LlamaIndex
Settings.llm = llamaindex_llm(client, model="gpt-4o")
```

See [Framework Integration Cookbook](../../docs/guides/framework-integrations.md) for full examples.

## Quick Start

### Agent / Proxy Usage

Route LLM requests through the gateway:

```python
from trueflow import TrueFlowClient

# Reads TRUEFLOW_API_KEY and TRUEFLOW_GATEWAY_URL from environment
client = TrueFlowClient()

# Check gateway health
print(client.health())  # -> {"status": "ok", ...}

# Use OpenAI's SDK — requests go through the gateway
oai = client.openai()
response = oai.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "Hello!"}],
)
```

### Admin / Management Usage

Manage tokens, credentials, policies, and view audit logs:

```python
from trueflow import TrueFlowClient

# Reads TRUEFLOW_ADMIN_KEY from environment
admin = TrueFlowClient.admin()

# Credentials
cred = admin.credentials.create(name="prod-openai", provider="openai", secret="sk-...")
creds = admin.credentials.list()  # → List[Credential]

# Tokens
token = admin.tokens.create(
    name="billing-bot",
    credential_id=cred["id"],
    upstream_url="https://api.openai.com",
    circuit_breaker={"enabled": True, "failure_threshold": 5},  # optional
)
api_key = token["token_id"]  # → "tf_v1_..."

tokens = admin.tokens.list()           # → List[Token]
admin.tokens.revoke(api_key)           # Soft-delete

# Circuit Breaker — read/update per-token CB config at runtime
config = admin.tokens.get_circuit_breaker(api_key)
admin.tokens.set_circuit_breaker(api_key, enabled=False)           # disable
admin.tokens.set_circuit_breaker(
    api_key, enabled=True, failure_threshold=3, recovery_cooldown_secs=30
)

# Upstream Health — view CB state of all tracked upstreams
health = admin.tokens.upstream_health()
# → [{"token_id": ..., "url": ..., "is_healthy": True, "failure_count": 0, ...}]

# Policies — using fluent DSL
from trueflow.policy import PolicyBuilder
rules = PolicyBuilder().when("prompt", "contains", "ignore instructions").deny("prompt injection detected").build()

policy = admin.policies.create(
    name="secure-agents",
    mode="enforce",
    rules=rules,
    retry={"max_retries": 3, "base_delay_ms": 500, "max_backoff_ms": 10000},
)
admin.policies.update(policy["id"], mode="shadow")
admin.policies.delete(policy["id"])

# Webhooks
webhook = admin.webhooks.create(url="https://mylogger.com/webhook", events=["token.created"])
admin.webhooks.test(webhook["url"])

# Audit logs auto-pagination
for log in admin.audit.list_all():
    print(f"{log.method} {log.path} -> {log.upstream_status}")

# HITL approvals
pending = admin.approvals.list()       # → List[ApprovalRequest]
admin.approvals.approve(pending[0].id)
admin.approvals.reject(pending[1].id)

# Prompt Management
prompt = admin.prompts.create(name="Customer Support Agent", folder="/support")
admin.prompts.create_version(
    prompt["id"],
    model="gpt-4o",
    messages=[
        {"role": "system", "content": "You help {{user_name}} with {{topic}}."},
        {"role": "user",   "content": "{{question}}"},
    ],
    commit_message="Initial version",
)
admin.prompts.deploy(prompt["id"], version=1, label="production")

# Render with variable substitution (result is cached client-side for 60s)
payload = admin.prompts.render(
    "customer-support-agent",
    variables={"user_name": "Alice", "topic": "billing", "question": "Where is my invoice?"},
    label="production",
)
# payload is OpenAI-compatible — pass directly to openai.chat.completions.create(**payload)

# Adjust cache TTL or clear it
custom = admin.prompts  # default 60s TTL, configurable per-resource
custom.invalidate("customer-support-agent")  # clear one slug
custom.clear_cache()                          # clear all

# A/B Experiments
exp = admin.experiments.create(
    name="gpt4o-vs-claude",
    variants=[
        {"name": "control",   "weight": 50, "model": "gpt-4o"},
        {"name": "treatment", "weight": 50, "model": "claude-3-5-sonnet-20241022"},
    ],
)
results = admin.experiments.results(exp["id"])  # per-variant metrics
admin.experiments.update(exp["id"], variants=[  # adjust weights mid-experiment
    {"name": "control",   "weight": 30, "model": "gpt-4o"},
    {"name": "treatment", "weight": 70, "model": "claude-3-5-sonnet-20241022"},
])
admin.experiments.stop(exp["id"])               # done
```

### Async Usage

```python
from trueflow import AsyncClient

async with AsyncClient() as client:
    oai = client.openai()
    tokens = await client.tokens.list()
```

## Error Handling

```python
from trueflow import TrueFlowClient
from trueflow.exceptions import (
    AuthenticationError,  # 401
    NotFoundError,        # 404
    RateLimitError,       # 429
    ValidationError,      # 422
    GatewayError,         # 5xx
    TrueFlowError,          # Base class
)

admin = TrueFlowClient.admin(admin_key="...")

try:
    admin.tokens.revoke("nonexistent")
except NotFoundError as e:
    print(f"Token not found: {e.message}")
    print(f"Status: {e.status_code}")
except TrueFlowError as e:
    print(f"Unexpected error: {e}")
```

## Models

List methods return typed Pydantic models:

| Model | Fields |
| :--- | :--- |
| `Token` | `id`, `name`, `credential_id`, `upstream_url`, `is_active`, `policy_ids`, `scopes` |
| `Credential` | `id`, `name`, `provider`, `created_at` |
| `Policy` | `id`, `name`, `mode`, `rules` |
| `AuditLog` | `id`, `method`, `path`, `upstream_status`, `response_latency_ms`, `agent_name`, ... |
| `ApprovalRequest` | `id`, `token_id`, `status`, `request_summary`, `expires_at` |
| `ApprovalDecision` | `id`, `status`, `updated` |

## License

MIT
