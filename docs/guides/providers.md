# Supported Providers

TrueFlow supports **10 LLM providers** with automatic format translation. Send requests in OpenAI format — the gateway detects the provider and translates on the fly.

---

## Provider Detection

The gateway detects the provider from the **model name prefix** first, then falls back to **URL matching**. No configuration needed for standard providers.

| Provider | Model Prefixes | Example Models |
|----------|---------------|----------------|
| **OpenAI** | `gpt-*`, `o1-*`, `o3-*`, `o4-*`, `chatgpt-*`, `dall-e-*`, `tts-*`, `whisper-*` | `gpt-4o`, `gpt-4o-mini`, `o3-mini` |
| **Anthropic** | `claude-*` | `claude-3-5-sonnet-20241022`, `claude-3-haiku` |
| **Google Gemini** | `gemini-*` | `gemini-2.0-flash`, `gemini-1.5-pro` |
| **Azure OpenAI** | *(URL-based)* | Same as OpenAI models, deployed as Azure endpoints |
| **Amazon Bedrock** | `anthropic.*`, `meta.*`, `amazon.*`, `cohere.*`, `mistral.*`, `ai21.*` | `anthropic.claude-3-sonnet`, `meta.llama3-70b` |
| **Groq** | `llama-*`, `mixtral-*` (via Groq URL) | `llama-3.1-70b-versatile` |
| **Mistral** | `mistral-*`, `codestral-*`, `open-mistral-*`, `pixtral-*` | `mistral-large-latest` |
| **Together AI** | `meta-llama/*`, `mistralai/*`, `Qwen/*` | `meta-llama/Meta-Llama-3.1-70B` |
| **Cohere** | `command-*` | `command-r-plus`, `command-r` |
| **Ollama** | *(URL-based, any model name)* | `llama3`, `codellama`, `mixtral` |

---

## Provider Details

### OpenAI

- **Auth**: Bearer token (`Authorization: Bearer sk-...`)
- **Credential setup**: Store `sk-...` key with `injection_mode: header`, `injection_header: Authorization`
- **Streaming**: SSE ✅ — native OpenAI format, zero translation needed
- **Multimodal**: Vision ✅, Audio ✅, Embeddings ✅
- **Tool calls**: ✅ Native support
- **URL format**: `https://api.openai.com/v1/chat/completions`

### Anthropic

- **Auth**: API key (`x-api-key: sk-ant-...`)
- **Credential setup**: Store `sk-ant-...` with `injection_mode: header`, `injection_header: x-api-key`
- **Streaming**: SSE ✅ — Anthropic format auto-translated to OpenAI format
- **Multimodal**: Vision ✅ (base64 images in content blocks)
- **Tool calls**: ✅ Translated between OpenAI and Anthropic formats
- **Auto-injected headers**: `anthropic-version: 2023-06-01`
- **URL format**: `https://api.anthropic.com/v1/messages`

### Google Gemini

- **Auth**: API key (query parameter `?key=...`)
- **Credential setup**: Store API key with `injection_mode: query`, `injection_header: key`
- **Streaming**: SSE ✅ — Gemini format auto-translated to OpenAI format
- **Multimodal**: Vision ✅ (inline images via `inlineData` or URLs via `fileData`)
- **Tool calls**: ✅ Translated between OpenAI `function_call` and Gemini `functionCall`
- **URL rewrite**: `{base}/v1beta/models/{model}:generateContent` (or `:streamGenerateContent`)

### Azure OpenAI

- **Auth**: API key (`api-key: ...`) or Bearer token
- **Credential setup**: Store Azure key with `injection_mode: header`, `injection_header: api-key`
- **Streaming**: SSE ✅ — OpenAI-compatible format
- **URL rewrite**: `{endpoint}/openai/deployments/{model}/chat/completions?api-version=2024-05-01-preview`
- **Notes**: Same request/response format as OpenAI — zero translation needed

### Amazon Bedrock

- **Auth**: AWS SigV4 signing
- **Credential setup**: Store `ACCESS_KEY_ID:SECRET_ACCESS_KEY` with `injection_mode: sigv4`
- **Streaming**: Binary event stream ✅ — `application/vnd.amazon.eventstream` auto-decoded and translated to OpenAI SSE
- **Request translation**: OpenAI → Bedrock Converse API format
- **Response translation**: Bedrock → OpenAI format with usage
- **Tool calls**: ✅ Translated between OpenAI `tool_calls` and Bedrock `toolUse`/`toolResult`
- **CRC32 validation**: ✅ Both prelude and message CRCs verified
- **URL format**: `https://bedrock-runtime.{region}.amazonaws.com/model/{model}/converse` (or `/converse-stream`)

### Groq

- **Auth**: Bearer token
- **Credential setup**: Store API key with default Bearer injection
- **Streaming**: SSE ✅ — OpenAI-compatible format
- **Notes**: OpenAI-compatible API — zero translation needed
- **URL format**: `https://api.groq.com/openai/v1/chat/completions`

### Mistral

- **Auth**: Bearer token
- **Credential setup**: Store API key with default Bearer injection
- **Streaming**: SSE ✅ — OpenAI-compatible format
- **Notes**: OpenAI-compatible API — zero translation needed
- **URL format**: `https://api.mistral.ai/v1/chat/completions`

### Together AI

- **Auth**: Bearer token
- **Credential setup**: Store API key with default Bearer injection
- **Streaming**: SSE ✅ — OpenAI-compatible format
- **Notes**: Model names use `/` separator (e.g., `meta-llama/Meta-Llama-3.1-70B`)
- **URL format**: `https://api.together.xyz/v1/chat/completions`

### Cohere

- **Auth**: Bearer token
- **Credential setup**: Store API key with default Bearer injection
- **Streaming**: SSE ✅ — OpenAI-compatible format
- **Notes**: Uses OpenAI-compatible endpoint
- **URL format**: `https://api.cohere.com/compatibility/v1/chat/completions`

### Ollama

- **Auth**: None (local server)
- **Credential setup**: No credential needed — use passthrough mode
- **Streaming**: SSE ✅ — OpenAI-compatible format
- **Notes**: Runs locally, default port 11434
- **URL format**: `http://localhost:11434/v1/chat/completions`

---

## Feature Matrix

| Feature | OpenAI | Anthropic | Gemini | Azure | Bedrock | Groq | Mistral | Together | Cohere | Ollama |
|---------|:------:|:---------:|:------:|:-----:|:-------:|:----:|:-------:|:--------:|:------:|:------:|
| Chat completions | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Streaming | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Tool/function calls | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — | ✅ |
| Vision/multimodal | ✅ | ✅ | ✅ | ✅ | ✅ | — | ✅ | — | — | ✅ |
| Auto-translation | — | ✅ | ✅ | — | ✅ | — | — | — | — | — |
| Cost tracking | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |

> **Auto-translation** means the gateway converts between OpenAI and native formats. Providers without auto-translation use OpenAI-compatible APIs natively.
