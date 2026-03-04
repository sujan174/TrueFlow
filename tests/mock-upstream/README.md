# TrueFlow Mock Upstream

Simulates OpenAI, Anthropic, Gemini, Azure Content Safety, AWS Comprehend, LlamaGuard, and a webhook receiver — all in one FastAPI service on port 9000.

## Endpoints

| Provider | Route |
|----------|-------|
| OpenAI / LlamaGuard | `POST /v1/chat/completions` |
| Anthropic | `POST /v1/messages` |
| Gemini (sync) | `POST /v1beta/models/{model}:generateContent` |
| Gemini (stream) | `POST /v1beta/models/{model}:streamGenerateContent` |
| Azure Content Safety | `POST /contentsafety/text:analyze` |
| AWS Comprehend | `POST /comprehend/detect-toxic` |
| Webhook receiver | `POST /webhook` |
| Webhook history | `GET  /webhook/history` |
| Health | `GET  /healthz` |

## Control Headers

Add these to any request to change behaviour per-call:

| Header | Effect |
|--------|--------|
| `x-mock-latency-ms: 500` | Add 500ms artificial latency |
| `x-mock-flaky: true` | 50% chance of HTTP 500 (circuit breaker testing) |
| `x-mock-status: 429` | Force any HTTP status code |
| `x-mock-drop-mid-stream: true` | Drop SSE after 2nd chunk (stream error testing) |
| `x-mock-tool-call: true` | Return a function/tool call instead of text |
| `x-mock-content: hello` | Override the response text content |

## Content Safety Trigger

Include the literal string **`harm_trigger`** anywhere in the request body text to make all guardrail endpoints return a flagged/harmful result.

## Run locally

```bash
cd tests/mock-upstream
pip install -r requirements.txt
python server.py
# → http://localhost:9000
```

Or via Docker Compose:

```bash
docker compose up mock-upstream
```
