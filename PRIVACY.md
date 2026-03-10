# Privacy Policy

TrueFlow sits directly in the path of your most sensitive data: your LLM prompts, your customers' queries, and your proprietary context. 

We built TrueFlow with a "Privacy-First, Local-First" architecture. This document explains exactly what data flows through TrueFlow, what is logged, and how we handle your data.

## Self-Hosted vs. Cloud

**If you self-host TrueFlow:**
You own 100% of your data. TrueFlow runs entirely within your infrastructure (VPC). No telemetry, prompts, metadata, or API keys are ever sent back to us. You control the database, the Redis cache, and the logs.

**If you use TrueFlow Cloud:**
The following policies apply to our hosted infrastructure.

---

## 1. What We Log (By Default)

We believe in logging only what is necessary to route, rate-limit, and bill your requests. By default, **we do not log the contents of your prompts or model responses.**

When a request passes through TrueFlow, we log the following **metadata**:
* Timestamp
* Token ID (the `tf_v1_...` key used)
* Upstream Provider & Model (e.g., `openai`, `gpt-4o`)
* Token Usage (Prompt tokens, Completion tokens, Total tokens)
* Request Latency (Time to first byte, Total duration)
* HTTP Status Code
* Cost estimate

*This metadata is stored in our database for 30 days to power your analytics dashboard.*

## 2. What We DO NOT Log (By Default)

**By default, the actual content of your requests is ephemeral.** It exists in memory just long enough to proxy it to the LLM provider and is immediately discarded.

We **DO NOT** log:
* The `messages` array (your prompts, system instructions, or context)
* The model's `choices` or streamed response content
* Custom headers you send (unless explicitly configured)
* PII (Personally Identifiable Information)

## 3. Opt-In Content Logging (Audit Logs)

If you enable the **Audit Logging** feature on a specific Token or Policy, TrueFlow will store the full request and response payloads.

* **Why?** For debugging, compliance, or fine-tuning datasets.
* **Control:** You have complete control over this. You can enable/disable it per-token.
* **Storage:** Audit logs are encrypted at rest.
* **Retention:** Audit logs are automatically purged after your configured retention period (default: 7 days).

## 4. API Keys and Credentials

When you provide us with your provider API keys (e.g., your OpenAI `sk-...` key):
* They are immediately encrypted using **AES-256-GCM envelope encryption**.
* The Master Key (KEK) is never stored alongside the database.
* The keys are only decrypted in-memory at the exact moment a request is proxied to the provider.
* We cannot read your plaintext API keys. Our employees cannot read your plaintext API keys.

## 5. PII Redaction

If you enable PII Redaction policies (e.g., stripping SSNs or Credit Cards), the redaction happens **in memory before the request is sent to the provider**.

The redacted data (e.g., `[REDACTED_SSN]`) is what gets sent to OpenAI/Anthropic, and is what gets saved to your Audit Logs (if Audit Logging is enabled). The original PII is never stored by TrueFlow.

## 6. Subprocessors

TrueFlow Cloud relies on the following infrastructure providers:
* **AWS / GCP:** Cloud hosting and database infrastructure (SOC2 Type II compliant).
* **Stripe:** Payment processing (we do not store your credit card numbers).

## 7. Compliance & Requests

If you need to delete your data, export your audit logs, or have compliance questions (SOC2, GDPR, HIPAA), please contact us at **privacy@trueflow.ai**.
