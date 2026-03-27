# TrueFlow Model Catalog

This document lists all supported models by provider. It's designed to be easily updated when providers release new models.

**Last Updated**: 2026-03-27

## Updating This Catalog

To update the model catalog:

1. **OpenAI**: Check https://platform.openai.com/docs/models
2. **Anthropic**: Check https://docs.anthropic.com/en/docs/about-claude/models
3. **Google Gemini**: Check https://ai.google.dev/gemini-api/docs/models/gemini
4. **Groq**: Check https://console.groq.com/docs/models
5. **Mistral**: Check https://docs.mistral.ai/getting-started/models/models_overview/
6. **Cohere**: Check https://docs.cohere.com/docs/models
7. **Together AI**: Check https://docs.together.ai/docs/serverless-models

Alternatively, fetch the latest from LiteLLM's model catalog:
```bash
curl -s 'https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json' | jq 'keys' > models-latest.txt
```

---

## OpenAI

### Model Families

| Family | Pattern | Description |
|--------|---------|-------------|
| GPT-4o | `gpt-4o*` | Latest multimodal models |
| GPT-4 | `gpt-4*` | Advanced reasoning |
| GPT-3.5 | `gpt-3.5*` | Fast, affordable |
| O1 | `o1-*` | Reasoning models |
| O3 | `o3-*` | Latest reasoning |

### Current Models (2026-03)

| Model ID | Description |
|----------|-------------|
| `gpt-4o` | Most capable GPT-4o model |
| `gpt-4o-mini` | Fast, affordable GPT-4o |
| `gpt-4o-audio-preview` | Audio capabilities |
| `gpt-4o-realtime-preview` | Real-time voice |
| `gpt-4-turbo` | GPT-4 Turbo |
| `gpt-4` | Original GPT-4 |
| `gpt-4-32k` | Extended context |
| `gpt-3.5-turbo` | Fast, cheap model |
| `gpt-3.5-turbo-16k` | Extended context |
| `o1` | Advanced reasoning |
| `o1-mini` | Fast reasoning |
| `o1-pro` | Pro tier reasoning |
| `o3-mini` | Latest reasoning model |
| `chatgpt-4o-latest` | ChatGPT-4o |

---

## Anthropic

### Model Families

| Family | Pattern | Description |
|--------|---------|-------------|
| Claude 4 | `claude-4-*` | Latest Claude models |
| Claude 3.7 | `claude-3-7-*` | Enhanced Claude 3.5 |
| Claude 3.5 | `claude-3-5-*` | Fast, capable |
| Claude 3 | `claude-3-*` | Original Claude 3 |

### Current Models (2026-03)

| Model ID | Description |
|----------|-------------|
| `claude-opus-4-20250514` | Claude 4 Opus |
| `claude-sonnet-4-20250514` | Claude 4 Sonnet |
| `claude-4-sonnet` | Claude 4 Sonnet alias |
| `claude-4-opus` | Claude 4 Opus alias |
| `claude-3-7-sonnet-20250219` | Claude 3.7 Sonnet |
| `claude-3-5-sonnet-20241022` | Claude 3.5 Sonnet v2 |
| `claude-3-5-haiku-20241022` | Claude 3.5 Haiku |
| `claude-3-opus-20240229` | Claude 3 Opus |
| `claude-3-haiku-20240307` | Claude 3 Haiku |

---

## Google Gemini

### Model Families

| Family | Pattern | Description |
|--------|---------|-------------|
| Gemini 3 | `gemini-3*` | Latest Gemini |
| Gemini 2.5 | `gemini-2.5*` | Advanced multimodal |
| Gemini 2 | `gemini-2*` | Second generation |
| Gemini 1.5 | `gemini-1.5*` | Long context |
| Gemini 1 | `gemini-1*` | Original Gemini |

### Current Models (2026-03)

| Model ID | Description |
|----------|-------------|
| `gemini-3.1-pro-preview` | Gemini 3.1 Pro |
| `gemini-3-flash-preview` | Gemini 3 Flash |
| `gemini-2.5-flash` | Gemini 2.5 Flash |
| `gemini-2.5-pro` | Gemini 2.5 Pro |
| `gemini-2.0-flash` | Gemini 2.0 Flash |
| `gemini-1.5-pro` | Gemini 1.5 Pro |
| `gemini-1.5-flash` | Gemini 1.5 Flash |
| `gemini-1.0-pro` | Gemini 1.0 Pro |

---

## Groq

Groq hosts many open-source models with ultra-fast inference.

### Model Patterns

| Pattern | Models |
|---------|--------|
| `llama-*` | Meta Llama models |
| `mixtral-*` | Mistral Mixtral |
| `gemma*` | Google Gemma |
| `deepseek-*` | DeepSeek models |
| `qwen-*` | Alibaba Qwen |
| `whisper-*` | OpenAI Whisper |

### Current Models (2026-03)

| Model ID | Description |
|----------|-------------|
| `llama-3.3-70b-versatile` | Llama 3.3 70B |
| `llama-3.1-8b-instant` | Llama 3.1 8B |
| `llama-3.2-11b-vision-preview` | Llama 3.2 Vision |
| `mixtral-8x7b-32768` | Mixtral 8x7B |
| `gemma2-9b-it` | Gemma 2 9B |
| `deepseek-r1-distill-llama-70b` | DeepSeek R1 |
| `qwen-2.5-32b` | Qwen 2.5 32B |
| `whisper-large-v3` | Whisper Large V3 |

---

## Mistral AI

### Model Families

| Family | Pattern | Description |
|--------|---------|-------------|
| Mistral Large | `mistral-large*` | Most capable |
| Mistral Medium | `mistral-medium*` | Balanced |
| Mistral Small | `mistral-small*` | Fast |
| Codestral | `codestral*` | Code generation |
| Devstral | `devstral*` | Code agents |

### Current Models (2026-03)

| Model ID | Description |
|----------|-------------|
| `mistral-large-3-25-12` | Mistral Large 3 |
| `mistral-medium-3-1-25-08` | Mistral Medium 3.1 |
| `mistral-small-4-25-05` | Mistral Small 4 |
| `devstral-2-25-12` | Devstral 2 |
| `codestral-2501` | Codestral Latest |
| `mistral-embed` | Embedding model |

---

## Cohere

### Model Families

| Family | Pattern | Description |
|--------|---------|-------------|
| Command A | `command-a*` | Latest command |
| Command R | `command-r*` | RAG optimized |
| Command | `command*` | General purpose |
| Aya | `aya*` | Multilingual |
| Embed | `embed-*` | Embeddings |
| Rerank | `rerank-*` | Reranking |

### Current Models (2026-03)

| Model ID | Description |
|----------|-------------|
| `command-a-03-2025` | Command A |
| `command-r-plus` | Command R Plus |
| `command-r` | Command R |
| `aya-expanse-32` | Aya Expanse 32B |
| `embed-english-v3.0` | English Embeddings |
| `rerank-english-v3.0` | English Reranking |

---

## Together AI

Together AI hosts many open-source models. See their full list at:
https://docs.together.ai/docs/serverless-models

### Popular Models

| Model ID | Description |
|----------|-------------|
| `meta-llama/Llama-3.3-70B-Instruct-Turbo` | Llama 3.3 70B |
| `Qwen/Qwen3-235B-A22B` | Qwen3 235B |
| `deepseek-ai/DeepSeek-V3` | DeepSeek V3 |
| `mistralai/Mistral-Small-24B-Instruct-2501` | Mistral Small 24B |

---

## Provider Detection

The gateway auto-detects providers by model name prefix:

| Prefix | Provider |
|--------|----------|
| `gpt-*`, `o1-*`, `o3-*` | OpenAI |
| `claude-*` | Anthropic |
| `gemini-*` | Google Gemini |
| `llama-*`, `mixtral-*` | Groq (default) |
| `mistral-*`, `codestral-*` | Mistral AI |
| `command-*` | Cohere |

---

## Syncing Updates

When updating this catalog:

1. Update the **Last Updated** date
2. Add new models to the appropriate provider section
3. Update the SDK `Providers` class in `types.py` if model patterns change
4. Run tests to verify model detection still works
5. Update the dashboard's provider presets if needed