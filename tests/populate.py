#!/usr/bin/env python3
"""
TrueFlow Dashboard Population — SDK Edition
==========================================
Uses the TrueFlow Python SDK to populate the dashboard with realistic data,
then generates traffic via token OpenAI clients to fill analytics.
"""

import sys, os, random, time

# Add the SDK to the path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "sdk", "python"))

from trueflow import TrueFlowClient

GW = "http://localhost:8443"
ADMIN_KEY = "trueflow-admin-test"
MOCK_URL = "http://mock-upstream:9000"  # Docker network (gateway appends /v1/...)

# ── Admin client ─────────────────────────────────────────────────────
admin = TrueFlowClient.admin(admin_key=ADMIN_KEY, gateway_url=GW)

print("=" * 60)
print("  TrueFlow Dashboard Population (SDK)")
print("=" * 60)

# Health check
if not admin.is_healthy():
    print("❌ Gateway not reachable"); sys.exit(1)
print("✅ Gateway healthy")


# ── 1. Clean existing data ───────────────────────────────────────────
print("\n🧹 Cleaning existing data...")

for t in admin.tokens.list():
    admin.tokens.revoke(t.id); print(f"   Revoked token: {t.name}")

for p in admin.policies.list():
    admin.policies.delete(p.id); print(f"   Deleted policy: {p.name}")

for pr in admin.prompts.list():
    admin.prompts.delete(pr["id"]); print(f"   Deleted prompt: {pr['name']}")

for s in admin.services.list():
    admin.services.delete(s.id); print(f"   Deleted service: {s.name}")

# Credentials — delete via raw API (SDK doesn't have delete method)
resp = admin._http.get("/api/v1/credentials")
if resp.status_code == 200:
    for c in resp.json():
        admin._http.delete(f"/api/v1/credentials/{c['id']}")
        print(f"   Deleted credential: {c['name']}")

print("   ✅ Clean slate\n")


# ── 2. Create credentials ────────────────────────────────────────────
print("🔐 Creating vault credentials...")
creds = {}

for name, provider, secret in [
    ("openai-prod",    "openai",    "sk-mock-openai-key-prod-12345"),
    ("anthropic-prod", "anthropic", "sk-ant-mock-anthropic-key-67890"),
    ("gemini-prod",    "google",    "AIzaSy-mock-gemini-key-abcdef"),
    ("openai-staging", "openai",    "sk-mock-openai-key-staging-99999"),
]:
    r = admin.credentials.create(name=name, provider=provider, secret=secret)
    creds[name] = r.id
    print(f"   ✅ {name} ({provider}) → {r.id[:8]}...")


# ── 3. Create upstream services ───────────────────────────────────────
print("\n🌐 Creating upstream services...")

for name, stype, desc, cred_key in [
    ("OpenAI",    "openai",    "OpenAI GPT models via mock",  "openai-prod"),
    ("Anthropic", "anthropic", "Claude models via mock",      "anthropic-prod"),
    ("Google AI", "google",    "Gemini models via mock",      "gemini-prod"),
]:
    admin.services.create(
        name=name, base_url=MOCK_URL, description=desc,
        service_type=stype, credential_id=creds.get(cred_key),
    )
    print(f"   ✅ {name} → {MOCK_URL}")


# ── 4. Create agent tokens ───────────────────────────────────────────
print("\n🔑 Creating agent tokens...")
tokens = []

for name, cred_key in [
    ("production-chatbot", "openai-prod"),
    ("staging-api",        "openai-staging"),
    ("data-pipeline",      "anthropic-prod"),
    ("mobile-app-v2",      "openai-prod"),
    ("internal-agent",     "openai-prod"),
]:
    t = admin.tokens.create(
        name=name, upstream_url=MOCK_URL,
        credential_id=creds.get(cred_key),
    )
    tokens.append(t)
    print(f"   ✅ {name} → {t.token_id[:25]}...")


# ── 5. Create policies ───────────────────────────────────────────────
print("\n📋 Creating policies...")

policy_defs = [
    ("Rate Limit — Free Tier", [{"when": {"field": "token.name", "op": "contains", "value": "staging"}, "then": {"action": "rate_limit", "rpm": 30}}]),
    ("Cost Cap — Mobile",      [{"when": {"field": "token.name", "op": "eq", "value": "mobile-app-v2"}, "then": {"action": "deny", "status": 429, "message": "Budget exceeded"}}]),
    ("Model Override — Dev",   [{"when": {"field": "header.x-env", "op": "eq", "value": "dev"}, "then": {"action": "override_model", "model": "gpt-4o-mini"}}]),
    ("Cache — Repeat Queries", [{"when": {"field": "request.method", "op": "eq", "value": "POST"}, "then": {"action": "cache", "ttl_seconds": 300}}]),
]

for name, rules in policy_defs:
    try:
        # Use raw HTTP to avoid SDK sending mode="enforce" which gateway rejects
        resp = admin._http.post("/api/v1/policies", json={"name": name, "rules": rules})
        if resp.status_code < 400:
            print(f"   ✅ {name}")
        else:
            print(f"   ⚠️  {name}: {resp.status_code}")
    except Exception as e:
        print(f"   ⚠️  {name}: {e}")


# ── 6. Create prompts with versions ──────────────────────────────────
print("\n📝 Creating versioned prompts...")

prompt_defs = [
    ("Customer Support Agent",  "support-agent-v2",  "Primary support chatbot",     "/agents"),
    ("Code Review Assistant",   "code-review-v2",    "Reviews PRs for bugs",        "/agents"),
    ("Data Summarizer",         "data-summarizer-v2","Summarises long documents",   "/tools"),
]

for pname, slug, desc, folder in prompt_defs:
    model = random.choice(["gpt-4o", "claude-3-5-sonnet", "gpt-4o-mini"])
    pr = admin.prompts.create(name=pname, slug=slug, description=desc, folder=folder)
    pid = pr["id"]
    print(f"   ✅ {pname} ({pid[:8]}...)")

    admin.prompts.create_version(
        pid, model=model,
        messages=[
            {"role": "system", "content": f"You are a {pname.lower()}. Be helpful."},
            {"role": "user", "content": "{{input}}"},
        ],
        temperature=round(random.uniform(0.3, 0.9), 1),
        commit_message="Initial version",
    )
    print(f"      + v1 ({model})")


# ── 7. Generate traffic ──────────────────────────────────────────────
print("\n🚀 Generating traffic through gateway...")

messages_bank = [
    [{"role": "user", "content": "What is the capital of France?"}],
    [{"role": "user", "content": "Explain quantum computing simply."}],
    [{"role": "user", "content": "Write a Python function to sort a list."}],
    [{"role": "user", "content": "What are the benefits of microservices?"}],
    [{"role": "user", "content": "Explain the CAP theorem."}],
    [{"role": "user", "content": "How does HTTP/2 differ from HTTP/1.1?"}],
    [{"role": "system", "content": "You are a helpful assistant."}, {"role": "user", "content": "Draft a polite meeting decline email."}],
    [{"role": "system", "content": "You are a code reviewer."}, {"role": "user", "content": "Review: def add(a,b): return a+b"}],
]

models = ["gpt-4o", "gpt-4o-mini", "claude-3-5-sonnet", "gemini-1.5-flash"]
sessions = ["session-chatbot-001", "session-chatbot-002", "session-pipeline-001", "session-mobile-001"]

total, success, errors = 30, 0, 0

for i in range(total):
    tok = random.choice(tokens)
    try:
        client = TrueFlowClient(api_key=tok.token_id, gateway_url=GW)
        with client.trace(session_id=random.choice(sessions)):
            resp = client.post("/v1/chat/completions", json={
                "model": random.choice(models),
                "messages": random.choice(messages_bank),
                "max_tokens": random.randint(50, 200),
                "temperature": round(random.uniform(0.3, 1.0), 1),
            })
            if resp.status_code < 400:
                success += 1
            else:
                errors += 1
        client.close()
    except Exception as e:
        errors += 1

    if (i + 1) % 10 == 0:
        print(f"   📊 {i+1}/{total}: {success} ok, {errors} errors")

print(f"   ✅ Traffic: {success} successful, {errors} errors")

print("\n" + "=" * 60)
print("  ✅ Dashboard populated! Open http://localhost:3000")
print("=" * 60)
