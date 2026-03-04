#!/usr/bin/env python3
"""
TrueFlow Mock Upstream Server
===========================
Simulates OpenAI, Anthropic, Gemini, Azure Content Safety, AWS Comprehend,
LlamaGuard (as OpenAI-compatible), and a webhook receiver — all in one
FastAPI app on port 9000.

Control Headers (per-request behaviour)
---------------------------------------
x-mock-latency-ms: <N>       Add N ms of artificial latency before responding.
x-mock-flaky: true           50% chance of returning HTTP 500 (for circuit breaker / retry tests).
x-mock-status: <N>           Force a specific HTTP status code (e.g. 429, 503).
x-mock-drop-mid-stream: true Drop the SSE connection after the 2nd chunk (tests stream_bridge error path).
x-mock-tool-call: true       Return a function/tool-call instead of a text reply.
x-mock-content: <text>       Override the text content returned (default: "Mock response: …").

Trigger words for guardrails
-----------------------------
Any request body containing "harm_trigger" will be flagged as a violation
by all content-safety endpoints. Everything else is clean.
"""

from __future__ import annotations

import asyncio
import json
import random
import time
import uuid
from collections import deque
from typing import Any, AsyncIterator, Deque

from fastapi import FastAPI, Request, Response
from fastapi.responses import StreamingResponse, JSONResponse

app = FastAPI(title="TrueFlow Mock Upstream")

# ── In-memory webhook history ─────────────────────────────────────────────────
webhook_history: Deque[dict] = deque(maxlen=200)

# ── Helpers ───────────────────────────────────────────────────────────────────

HARM_TRIGGER = "harm_trigger"


def _now_ts() -> int:
    return int(time.time())


def _response_text(request_body: dict, override: str | None = None) -> str:
    if override:
        return override
    # Echo something vaguely based on the last user message
    messages = request_body.get("messages") or request_body.get("contents") or []
    last_content = ""
    if messages:
        last = messages[-1]
        # OpenAI / Anthropic style
        c = last.get("content") or ""
        if isinstance(c, list):
            c = " ".join(p.get("text", "") for p in c if isinstance(p, dict))
        # Gemini style
        if not c:
            parts = last.get("parts") or []
            c = " ".join(p.get("text", "") for p in parts if isinstance(p, dict))
        last_content = str(c)[:100]
    return f"Mock response to: {last_content}" if last_content else "Mock response"


def _is_harmful(body_text: str) -> bool:
    return HARM_TRIGGER in body_text.lower()


async def _apply_control_headers(request: Request) -> Response | None:
    """
    Apply x-mock-* control headers. Returns an early Response if the request
    should be short-circuited (flaky 500, forced status). Returns None otherwise.
    """
    # 0. Realistic base latency jitter (15-50ms) — simulates real LLM API RTT
    await asyncio.sleep(random.uniform(0.015, 0.050))

    # 1. Additional latency (on top of jitter)
    latency_ms = request.headers.get("x-mock-latency-ms")
    if latency_ms:
        try:
            await asyncio.sleep(int(latency_ms) / 1000.0)
        except ValueError:
            pass

    # 2. Forced status
    forced_status = request.headers.get("x-mock-status")
    if forced_status:
        try:
            code = int(forced_status)
            headers = {}
            if code == 429:
                headers = {
                    "x-ratelimit-limit-requests": "100",
                    "x-ratelimit-remaining-requests": "0",
                    "retry-after": "30",
                }
            return JSONResponse(
                status_code=code,
                content=_error_body(code, f"Mock forced status {code}"),
                headers=headers,
            )
        except ValueError:
            pass

    # 3. Flakiness
    if request.headers.get("x-mock-flaky", "").lower() == "true":
        if random.random() < 0.5:
            return JSONResponse(
                status_code=500,
                content=_error_body(500, "Mock flaky error"),
            )

    return None


def _error_body(status: int, message: str) -> dict:
    return {
        "error": {
            "message": message,
            "type": "mock_error",
            "code": str(status),
        }
    }


# ─────────────────────────────────────────────────────────────────────────────
# OpenAI  /v1/chat/completions
# ─────────────────────────────────────────────────────────────────────────────

@app.post("/v1/chat/completions")
async def openai_chat(request: Request):
    early = await _apply_control_headers(request)
    if early:
        return early

    body: dict = await request.json()
    is_streaming = body.get("stream", False)
    model = body.get("model", "gpt-4o")
    content_override = request.headers.get("x-mock-content")
    # Detect tool calls from BOTH header (direct mock tests) and body (gateway proxied)
    want_tool_call = (
        request.headers.get("x-mock-tool-call", "").lower() == "true"
        or bool(body.get("tools"))
    )
    # Echo actual tool name from request (not hardcoded) so tests can detect misrouting
    req_tool_name = "get_weather"
    tools_list = body.get("tools", [])
    if tools_list and isinstance(tools_list, list):
        try:
            req_tool_name = tools_list[0]["function"]["name"]
        except (KeyError, IndexError, TypeError):
            pass
    drop_mid_stream = request.headers.get("x-mock-drop-mid-stream", "").lower() == "true"

    # LlamaGuard detection — if model == llama-guard, delegate
    if "llama-guard" in model.lower():
        return await _llama_guard_response(request, body)

    text = _response_text(body, content_override)
    chat_id = f"chatcmpl-{uuid.uuid4().hex[:12]}"

    if is_streaming:
        return StreamingResponse(
            _openai_sse_stream(chat_id, model, text, want_tool_call, drop_mid_stream),
            media_type="text/event-stream",
            headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
        )

    if want_tool_call:
        message = {
            "role": "assistant",
            "content": None,
            "tool_calls": [{
                "id": f"call_{uuid.uuid4().hex[:8]}",
                "type": "function",
                "function": {
                    "name": req_tool_name,
                    "arguments": '{"location": "London", "unit": "celsius"}',
                },
            }],
        }
        finish_reason = "tool_calls"
    else:
        message = {"role": "assistant", "content": text}
        finish_reason = "stop"

    prompt_tokens = sum(len((m.get("content") or "").split()) for m in body.get("messages", []))
    completion_tokens = len(text.split())

    # Include debug echo of the received request — lets tests verify
    # that transforms, header injection, body field changes actually arrived.
    received_headers = dict(request.headers)
    # Exclude noisy auto-headers to keep response compact
    for k in ("host", "content-length", "accept", "accept-encoding", "connection"):
        received_headers.pop(k, None)

    return JSONResponse({
        "id": chat_id,
        "object": "chat.completion",
        "created": _now_ts(),
        "model": model,
        "system_fingerprint": f"fp_{uuid.uuid4().hex[:10]}",
        "choices": [{"index": 0, "message": message, "finish_reason": finish_reason}],
        "usage": {
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": prompt_tokens + completion_tokens,
        },
        "_debug": {
            "received_headers": received_headers,
            "received_body": body,
        },
    })


async def _openai_sse_stream(
    chat_id: str, model: str, text: str, tool_call: bool, drop: bool
) -> AsyncIterator[bytes]:
    words = text.split()
    chunk_num = 0

    if tool_call:
        # First chunk: tool call start
        yield _sse({"id": chat_id, "object": "chat.completion.chunk", "created": _now_ts(), "model": model,
                     "choices": [{"index": 0, "delta": {"role": "assistant", "tool_calls": [
                         {"index": 0, "id": f"call_{uuid.uuid4().hex[:8]}", "type": "function",
                          "function": {"name": "get_weather", "arguments": ""}}
                     ]}, "finish_reason": None}]})
        await asyncio.sleep(0.01)
        yield _sse({"id": chat_id, "object": "chat.completion.chunk", "created": _now_ts(), "model": model,
                     "choices": [{"index": 0, "delta": {"tool_calls": [
                         {"index": 0, "function": {"arguments": '{"location": "London"}'}}
                     ]}, "finish_reason": None}]})
        await asyncio.sleep(0.01)
        yield _sse({"id": chat_id, "object": "chat.completion.chunk", "created": _now_ts(), "model": model,
                     "choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls"}],
                     "usage": {"prompt_tokens": 10, "completion_tokens": 8, "total_tokens": 18}})
        yield b"data: [DONE]\n\n"
        return

    # First chunk: role
    yield _sse({"id": chat_id, "object": "chat.completion.chunk", "created": _now_ts(), "model": model,
                 "choices": [{"index": 0, "delta": {"role": "assistant"}, "finish_reason": None}]})
    await asyncio.sleep(0.01)

    # Content chunks
    for i, word in enumerate(words):
        chunk_num += 1
        content = word + (" " if i < len(words) - 1 else "")
        yield _sse({"id": chat_id, "object": "chat.completion.chunk", "created": _now_ts(), "model": model,
                     "choices": [{"index": 0, "delta": {"content": content}, "finish_reason": None}]})
        await asyncio.sleep(0.005)

        # Simulate mid-stream drop after 2nd content chunk
        if drop and chunk_num == 2:
            # Close without DONE — our stream_bridge should inject a structured error
            return

    # Final chunk with usage
    pt = 15
    ct = len(words)
    yield _sse({"id": chat_id, "object": "chat.completion.chunk", "created": _now_ts(), "model": model,
                 "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
                 "usage": {"prompt_tokens": pt, "completion_tokens": ct, "total_tokens": pt + ct}})
    yield b"data: [DONE]\n\n"


def _sse(data: dict) -> bytes:
    return f"data: {json.dumps(data)}\n\n".encode()


# ─────────────────────────────────────────────────────────────────────────────
# Anthropic  /v1/messages
# ─────────────────────────────────────────────────────────────────────────────

@app.post("/v1/messages")
async def anthropic_messages(request: Request):
    early = await _apply_control_headers(request)
    if early:
        return early

    # Anthropic requires anthropic-version header
    if "anthropic-version" not in request.headers:
        return JSONResponse(
            status_code=400,
            content={"type": "error", "error": {"type": "invalid_request_error",
                                                 "message": "Missing anthropic-version header"}},
        )

    body: dict = await request.json()
    is_streaming = body.get("stream", False)
    model = body.get("model", "claude-3-5-sonnet-20241022")
    content_override = request.headers.get("x-mock-content")
    # MU-1 fix: detect tool calls from body (not just header) like real Anthropic
    want_tool_call = (
        request.headers.get("x-mock-tool-call", "").lower() == "true"
        or bool(body.get("tools"))
    )
    drop_mid_stream = request.headers.get("x-mock-drop-mid-stream", "").lower() == "true"

    text = _response_text(body, content_override)
    msg_id = f"msg_{uuid.uuid4().hex[:12]}"
    input_tokens = sum(len(str(m.get("content", "")).split()) for m in body.get("messages", []))
    output_tokens = len(text.split())

    if is_streaming:
        return StreamingResponse(
            _anthropic_sse_stream(msg_id, model, text, input_tokens, output_tokens,
                                  want_tool_call, drop_mid_stream),
            media_type="text/event-stream",
            headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
        )

    if want_tool_call:
        content = [{
            "type": "tool_use",
            "id": f"toolu_{uuid.uuid4().hex[:10]}",
            "name": "get_weather",
            "input": {"location": "London"},
        }]
        stop_reason = "tool_use"
    else:
        content = [{"type": "text", "text": text}]
        stop_reason = "end_turn"

    return JSONResponse({
        "id": msg_id,
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": content,
        "stop_reason": stop_reason,
        "stop_sequence": None,
        "usage": {"input_tokens": input_tokens, "output_tokens": output_tokens},
    })


async def _anthropic_sse_stream(
    msg_id: str, model: str, text: str, input_tokens: int, output_tokens: int,
    tool_call: bool, drop: bool
) -> AsyncIterator[bytes]:
    def ant_event(event: str, data: dict) -> bytes:
        return f"event: {event}\ndata: {json.dumps(data)}\n\n".encode()

    yield ant_event("message_start", {
        "type": "message_start",
        "message": {
            "id": msg_id, "type": "message", "role": "assistant",
            "model": model, "content": [],
            "stop_reason": None, "stop_sequence": None,
            "usage": {"input_tokens": input_tokens, "output_tokens": 0},
        },
    })
    await asyncio.sleep(0.01)

    if tool_call:
        tool_id = f"toolu_{uuid.uuid4().hex[:10]}"
        yield ant_event("content_block_start", {
            "type": "content_block_start", "index": 0,
            "content_block": {"type": "tool_use", "id": tool_id, "name": "get_weather", "input": {}},
        })
        yield ant_event("content_block_delta", {
            "type": "content_block_delta", "index": 0,
            "delta": {"type": "input_json_delta", "partial_json": '{"location":'},
        })
        await asyncio.sleep(0.01)
        yield ant_event("content_block_delta", {
            "type": "content_block_delta", "index": 0,
            "delta": {"type": "input_json_delta", "partial_json": ' "London"}'},
        })
        yield ant_event("content_block_stop", {"type": "content_block_stop", "index": 0})
        yield ant_event("message_delta", {
            "type": "message_delta",
            "delta": {"stop_reason": "tool_use", "stop_sequence": None},
            "usage": {"output_tokens": output_tokens},
        })
        yield ant_event("message_stop", {"type": "message_stop"})
        return

    # Text streaming
    yield ant_event("content_block_start", {
        "type": "content_block_start", "index": 0,
        "content_block": {"type": "text", "text": ""},
    })

    words = text.split()
    for i, word in enumerate(words):
        content = word + (" " if i < len(words) - 1 else "")
        yield ant_event("content_block_delta", {
            "type": "content_block_delta", "index": 0,
            "delta": {"type": "text_delta", "text": content},
        })
        await asyncio.sleep(0.005)

        if drop and i == 1:
            return  # Mid-stream drop

    yield ant_event("content_block_stop", {"type": "content_block_stop", "index": 0})
    yield ant_event("message_delta", {
        "type": "message_delta",
        "delta": {"stop_reason": "end_turn", "stop_sequence": None},
        "usage": {"output_tokens": output_tokens},
    })
    yield ant_event("message_stop", {"type": "message_stop"})


# ─────────────────────────────────────────────────────────────────────────────
# Gemini  /v1beta/models/{model}:generateContent
#         /v1beta/models/{model}:streamGenerateContent
# ─────────────────────────────────────────────────────────────────────────────

@app.post("/v1beta/models/{model_id}:generateContent")
async def gemini_generate(model_id: str, request: Request):
    early = await _apply_control_headers(request)
    if early:
        return early

    body: dict = await request.json()
    content_override = request.headers.get("x-mock-content")
    want_tool_call = request.headers.get("x-mock-tool-call", "").lower() == "true"
    text = _response_text(body, content_override)

    prompt_tokens = 0
    for c in body.get("contents", []):
        for p in c.get("parts", []):
            prompt_tokens += len(p.get("text", "").split())
    completion_tokens = len(text.split())

    # MU-6 fix: echo actual tool name from request body
    req_tool_name = "get_weather"
    tools_list = body.get("tools") or body.get("functionDeclarations") or []
    if tools_list and isinstance(tools_list, list):
        try:
            req_tool_name = tools_list[0].get("function", tools_list[0]).get("name", req_tool_name)
        except (AttributeError, KeyError):
            pass

    if want_tool_call:
        parts = [{"functionCall": {"name": req_tool_name, "args": {"location": "London"}}}]
        finish_reason = "FUNCTION_CALL"  # MU-3 fix: real Gemini returns FUNCTION_CALL, not STOP
    else:
        parts = [{"text": text}]
        finish_reason = "STOP"

    return JSONResponse({
        "candidates": [{
            "content": {"role": "model", "parts": parts},
            "finishReason": finish_reason,
            "index": 0,
            "safetyRatings": [
                {"category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "probability": "NEGLIGIBLE"},
                {"category": "HARM_CATEGORY_HATE_SPEECH", "probability": "NEGLIGIBLE"},
                {"category": "HARM_CATEGORY_HARASSMENT", "probability": "NEGLIGIBLE"},
                {"category": "HARM_CATEGORY_DANGEROUS_CONTENT", "probability": "NEGLIGIBLE"},
            ],
        }],
        "usageMetadata": {
            "promptTokenCount": prompt_tokens,
            "candidatesTokenCount": completion_tokens,
            "totalTokenCount": prompt_tokens + completion_tokens,
        },
        "modelVersion": model_id,
    })


@app.post("/v1beta/models/{model_id}:streamGenerateContent")
async def gemini_stream(model_id: str, request: Request):
    early = await _apply_control_headers(request)
    if early:
        return early

    body: dict = await request.json()
    content_override = request.headers.get("x-mock-content")
    want_tool_call = request.headers.get("x-mock-tool-call", "").lower() == "true"
    drop_mid_stream = request.headers.get("x-mock-drop-mid-stream", "").lower() == "true"
    text = _response_text(body, content_override)

    prompt_tokens = 0
    for c in body.get("contents", []):
        for p in c.get("parts", []):
            prompt_tokens += len(p.get("text", "").split())

    return StreamingResponse(
        _gemini_sse_stream(model_id, text, prompt_tokens, want_tool_call, drop_mid_stream),
        media_type="text/event-stream",
        headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
    )


async def _gemini_sse_stream(
    model_id: str, text: str, prompt_tokens: int, tool_call: bool, drop: bool
) -> AsyncIterator[bytes]:
    """Gemini streams as line-delimited JSON arrays (each event is a JSON object on its own line)."""
    words = text.split()
    total_words = len(words)

    if tool_call:
        chunk = {
            "candidates": [{"content": {"role": "model",
                                         "parts": [{"functionCall": {"name": "get_weather",
                                                                      "args": {"location": "London"}}}]},
                             "finishReason": "STOP", "index": 0}],
            "usageMetadata": {"promptTokenCount": prompt_tokens, "candidatesTokenCount": 5,
                              "totalTokenCount": prompt_tokens + 5},
        }
        yield f"data: {json.dumps(chunk)}\n\n".encode()
        return

    for i, word in enumerate(words):
        chunk_text = word + (" " if i < total_words - 1 else "")
        is_last = i == total_words - 1
        chunk = {
            "candidates": [{"content": {"role": "model", "parts": [{"text": chunk_text}]},
                             "finishReason": "STOP" if is_last else None, "index": 0}],
            "usageMetadata": {"promptTokenCount": prompt_tokens, "candidatesTokenCount": i + 1,
                              "totalTokenCount": prompt_tokens + i + 1},
        }
        yield f"data: {json.dumps(chunk)}\n\n".encode()
        await asyncio.sleep(0.005)

        if drop and i == 1:
            return  # Mid-stream drop


# ─────────────────────────────────────────────────────────────────────────────
# Azure Content Safety  /contentsafety/text:analyze
# ─────────────────────────────────────────────────────────────────────────────

@app.post("/contentsafety/text:analyze")
async def azure_content_safety(request: Request):
    early = await _apply_control_headers(request)
    if early:
        return early

    # Requires Ocp-Apim-Subscription-Key
    if "ocp-apim-subscription-key" not in request.headers:
        return JSONResponse(status_code=401, content={
            "error": {"code": "Unauthorized", "message": "Missing subscription key"}
        })

    body: dict = await request.json()
    text = body.get("text", "")
    harmful = _is_harmful(text)

    severity = 6 if harmful else 0
    categories = body.get("categories", ["Hate", "Violence", "Sexual", "SelfHarm"])

    # MU-5 fix: flag ALL categories when harmful (not just the first)
    analysis = [
        {"category": cat, "severity": severity}
        for cat in categories
    ]

    return JSONResponse({
        "categoriesAnalysis": analysis,
        "blocklistsMatch": [],
    })


# ─────────────────────────────────────────────────────────────────────────────
# AWS Comprehend (proxy/mock)  /comprehend/detect-toxic
# ─────────────────────────────────────────────────────────────────────────────

@app.post("/comprehend/detect-toxic")
async def aws_comprehend(request: Request):
    early = await _apply_control_headers(request)
    if early:
        return early

    body: dict = await request.json()
    segments = body.get("TextSegments", [])
    text = " ".join(s.get("Text", "") for s in segments)
    harmful = _is_harmful(text)

    score_value = 0.97 if harmful else 0.02
    label_name = "HATE_SPEECH" if harmful else "PROFANITY"

    return JSONResponse({
        "ResultList": [{
            "Index": 0,
            "Labels": [{"Name": label_name, "Score": score_value}],
        }],
        "ErrorList": [],
        "ResponseMetadata": {"RequestId": str(uuid.uuid4()), "HTTPStatusCode": 200},
    })


# ─────────────────────────────────────────────────────────────────────────────
# LlamaGuard — uses OpenAI-compatible /v1/chat/completions with model=llama-guard
# Handled inside the openai_chat route via model detection
# ─────────────────────────────────────────────────────────────────────────────

async def _llama_guard_response(request: Request, body: dict) -> JSONResponse:
    messages = body.get("messages", [])
    # Check last user message
    last_user = ""
    for m in reversed(messages):
        if m.get("role") == "user":
            c = m.get("content", "")
            last_user = c if isinstance(c, str) else str(c)
            break

    harmful = _is_harmful(last_user)
    content = "unsafe\nO1: Violence" if harmful else "safe"

    chat_id = f"chatcmpl-{uuid.uuid4().hex[:12]}"
    return JSONResponse({
        "id": chat_id,
        "object": "chat.completion",
        "created": _now_ts(),
        "model": "llama-guard",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": content},
            "finish_reason": "stop",
        }],
        "usage": {"prompt_tokens": 50, "completion_tokens": 3, "total_tokens": 53},
    })


# ─────────────────────────────────────────────────────────────────────────────
# Webhook receiver  POST /webhook  GET /webhook/history
# ─────────────────────────────────────────────────────────────────────────────

@app.post("/webhook")
async def webhook_receive(request: Request):
    early = await _apply_control_headers(request)
    if early:
        return early

    try:
        payload = await request.json()
    except Exception:
        payload = {"raw": (await request.body()).decode(errors="replace")}

    entry = {
        "id": str(uuid.uuid4()),
        "received_at": _now_ts(),
        "headers": dict(request.headers),
        "payload": payload,
    }
    webhook_history.appendleft(entry)
    return JSONResponse({"status": "received", "id": entry["id"]}, status_code=200)


@app.get("/webhook/history")
async def webhook_history_get(limit: int = 50):
    return JSONResponse(list(webhook_history)[:limit])


@app.delete("/webhook/history")
async def webhook_history_clear():
    webhook_history.clear()
    return JSONResponse({"status": "cleared"})


# ─────────────────────────────────────────────────────────────────────────────
# OpenAI Embeddings  POST /v1/embeddings
# ─────────────────────────────────────────────────────────────────────────────

@app.post("/v1/embeddings")
async def openai_embeddings(request: Request):
    early = await _apply_control_headers(request)
    if early:
        return early

    body: dict = await request.json()
    model = body.get("model", "text-embedding-3-small")
    input_text = body.get("input", "")
    if isinstance(input_text, list):
        inputs = input_text
    else:
        inputs = [input_text]

    data = []
    for i, inp in enumerate(inputs):
        # Deterministic 1536-dim embedding (hash-based so same input → same vector)
        seed = hash(inp) % 10000
        rng = random.Random(seed)
        embedding = [round(rng.uniform(-1, 1), 6) for _ in range(1536)]
        data.append({"object": "embedding", "index": i, "embedding": embedding})

    total_tokens = sum(len(str(t).split()) for t in inputs)
    return JSONResponse({
        "object": "list",
        "data": data,
        "model": model,
        "usage": {"prompt_tokens": total_tokens, "total_tokens": total_tokens},
    })


# ─────────────────────────────────────────────────────────────────────────────
# OpenAI Audio  POST /v1/audio/transcriptions
# ─────────────────────────────────────────────────────────────────────────────

@app.post("/v1/audio/transcriptions")
async def openai_audio_transcriptions(request: Request):
    early = await _apply_control_headers(request)
    if early:
        return early

    # Accept multipart/form-data — just return a canned transcription
    form = await request.form()
    model = form.get("model", "whisper-1")
    language = form.get("language", "en")

    return JSONResponse({
        "text": f"Mock transcription of audio file in {language}. Model: {model}.",
    })


# ─────────────────────────────────────────────────────────────────────────────
# OpenAI Images  POST /v1/images/generations
# ─────────────────────────────────────────────────────────────────────────────

@app.post("/v1/images/generations")
async def openai_image_generations(request: Request):
    early = await _apply_control_headers(request)
    if early:
        return early

    body: dict = await request.json()
    n = body.get("n", 1)
    size = body.get("size", "1024x1024")
    prompt = body.get("prompt", "")

    data = []
    for i in range(n):
        data.append({
            "url": f"https://mock.trueflow.test/images/{uuid.uuid4().hex}.png",
            "revised_prompt": f"Mock image {i+1}: {prompt[:50]}",
        })

    return JSONResponse({"created": _now_ts(), "data": data})


# ─────────────────────────────────────────────────────────────────────────────
# OpenAI Models  GET /v1/models  GET /v1/models/{model_id}
# ─────────────────────────────────────────────────────────────────────────────

MOCK_MODELS = [
    {"id": "gpt-4o", "object": "model", "created": 1700000000, "owned_by": "mock"},
    {"id": "gpt-4o-mini", "object": "model", "created": 1700000000, "owned_by": "mock"},
    {"id": "text-embedding-3-small", "object": "model", "created": 1700000000, "owned_by": "mock"},
    {"id": "whisper-1", "object": "model", "created": 1700000000, "owned_by": "mock"},
    {"id": "dall-e-3", "object": "model", "created": 1700000000, "owned_by": "mock"},
]


@app.get("/v1/models")
async def openai_models_list(request: Request):
    early = await _apply_control_headers(request)
    if early:
        return early
    return JSONResponse({"object": "list", "data": MOCK_MODELS})


@app.get("/v1/models/{model_id}")
async def openai_model_detail(model_id: str, request: Request):
    early = await _apply_control_headers(request)
    if early:
        return early
    for m in MOCK_MODELS:
        if m["id"] == model_id:
            return JSONResponse(m)
    return JSONResponse(status_code=404, content={
        "error": {"message": f"Model '{model_id}' not found", "type": "invalid_request_error"}
    })


# ─────────────────────────────────────────────────────────────────────────────
# Health + info
# ─────────────────────────────────────────────────────────────────────────────

@app.get("/healthz")
async def health():
    return {"status": "ok", "service": "trueflow-mock-upstream"}


@app.get("/")
async def root():
    return {
        "service": "TrueFlow Mock Upstream",
        "endpoints": [
            "POST /v1/chat/completions           (OpenAI + LlamaGuard)",
            "POST /v1/messages                   (Anthropic)",
            "POST /v1beta/models/{m}:generateContent       (Gemini)",
            "POST /v1beta/models/{m}:streamGenerateContent (Gemini SSE)",
            "POST /contentsafety/text:analyze    (Azure Content Safety)",
            "POST /comprehend/detect-toxic       (AWS Comprehend)",
            "POST /webhook                       (Webhook receiver)",
            "GET  /webhook/history               (Captured webhooks)",
            "POST /v1/embeddings                 (OpenAI Embeddings)",
            "POST /v1/audio/transcriptions       (OpenAI Audio)",
            "POST /v1/images/generations         (OpenAI Images)",
            "GET  /v1/models                     (OpenAI Models list)",
            "GET  /v1/models/{id}                (OpenAI Model detail)",
            "GET  /healthz                       (Health check)",
            "GET  /.well-known/openid-configuration (OIDC Discovery)",
            "GET  /.well-known/jwks.json         (OIDC JWKS Public Keys)",
            "POST /oidc/mint                     (Test JWT minting endpoint)",
        ],
        "control_headers": {
            "x-mock-latency-ms": "Add N ms latency (on top of 15-50ms jitter)",
            "x-mock-flaky": "true = 50% chance 500",
            "x-mock-status": "Force HTTP status code (429 includes ratelimit headers)",
            "x-mock-drop-mid-stream": "true = drop SSE after 2nd chunk",
            "x-mock-tool-call": "true = return tool/function call",
            "x-mock-content": "Override response text",
        },
        "latency_jitter": "15-50ms random base latency on every request",
        "harm_trigger": f"Include '{HARM_TRIGGER}' in body text to trigger content safety flags",
    }


# ── OIDC Identity Provider Mock ───────────────────────────────────────────────
#
# A minimal OpenID Connect IdP for integration testing. Generates a fresh
# RSA-2048 key pair at startup, serves it via JWKS, and signs test JWTs.
#
# The issuer URL is http://localhost:9000 (or the MOCK_BASE_URL env var).
# Tests should register this issuer as an OIDC provider in the gateway.

try:
    from cryptography.hazmat.primitives.asymmetric import rsa, padding
    from cryptography.hazmat.primitives import serialization, hashes
    import jwt as pyjwt
    import base64 as _b64
    import struct

    def _int_to_base64url(n: int) -> str:
        """Convert a large integer to base64url-encoded bytes (big-endian, no padding)."""
        byte_length = (n.bit_length() + 7) // 8
        n_bytes = n.to_bytes(byte_length, "big")
        return _b64.urlsafe_b64encode(n_bytes).rstrip(b"=").decode()

    # Generate RSA key pair once at startup
    _oidc_private_key = rsa.generate_private_key(
        public_exponent=65537,
        key_size=2048,
    )
    _oidc_public_key = _oidc_private_key.public_key()
    _OIDC_KID = "mock-oidc-key-1"

    # Pre-build the JWKS response
    pub_numbers = _oidc_public_key.public_key().public_numbers() if hasattr(_oidc_public_key, 'public_key') else _oidc_public_key.public_numbers()
    _JWKS = {
        "keys": [
            {
                "kty": "RSA",
                "use": "sig",
                "alg": "RS256",
                "kid": _OIDC_KID,
                "n": _int_to_base64url(pub_numbers.n),
                "e": _int_to_base64url(pub_numbers.e),
            }
        ]
    }

    _OIDC_CRYPTO_AVAILABLE = True

except ImportError:
    _OIDC_CRYPTO_AVAILABLE = False
    _JWKS = {"keys": []}


MOCK_BASE_URL = __import__("os").getenv("MOCK_BASE_URL", "http://localhost:9000")


# ── OIDC Discovery Endpoint ────────────────────────────────────────────────────

@app.get("/.well-known/openid-configuration")
async def oidc_discovery():
    """OpenID Connect Discovery document."""
    return {
        "issuer": MOCK_BASE_URL,
        "authorization_endpoint": f"{MOCK_BASE_URL}/oidc/authorize",
        "token_endpoint": f"{MOCK_BASE_URL}/oidc/token",
        "jwks_uri": f"{MOCK_BASE_URL}/.well-known/jwks.json",
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "scopes_supported": ["openid", "profile", "email"],
        "claims_supported": ["sub", "iss", "aud", "exp", "iat", "email", "name",
                              "custom:trueflow_role", "custom:trueflow_scopes"],
    }


@app.get("/.well-known/jwks.json")
async def oidc_jwks():
    """JWKS endpoint — returns the RSA public key used to verify JWTs."""
    return _JWKS


# ── JWT Minting (test helper only — not a real OIDC endpoint) ─────────────────

@app.post("/oidc/mint")
async def oidc_mint(request: Request):
    """
    Mint a test JWT signed with the mock IdP's private key.

    Body (JSON):
        sub          - subject (user ID, required)
        email        - email claim (optional)
        role         - custom:trueflow_role claim (optional, default: 'admin')
        scopes       - custom:trueflow_scopes claim (optional, default: '*')
        audience     - aud claim (optional)
        expires_in   - token lifetime in seconds (optional, default: 3600)
        expired      - if true, token is already expired (default: false)
        bad_signature - if true, sign with a different key (invalid sig)
    """
    if not _OIDC_CRYPTO_AVAILABLE:
        return JSONResponse(
            {"error": "cryptography/PyJWT not installed in mock upstream"},
            status_code=503,
        )

    body = await request.json()
    sub = body.get("sub", "test-user-01")
    email = body.get("email", f"{sub}@example.com")
    role = body.get("role", "admin")
    scopes = body.get("scopes", "*")
    audience = body.get("audience")
    expires_in = int(body.get("expires_in", 3600))
    expired = body.get("expired", False)
    bad_sig = body.get("bad_signature", False)

    now = int(time.time())
    exp = (now - 3600) if expired else (now + expires_in)

    payload = {
        "iss": MOCK_BASE_URL,
        "sub": sub,
        "iat": now,
        "exp": exp,
        "email": email,
        "name": f"Test User ({sub})",
        "custom:trueflow_role": role,
        "custom:trueflow_scopes": scopes,
    }
    if audience:
        payload["aud"] = audience

    headers = {"kid": _OIDC_KID}

    if bad_sig:
        # Sign with a freshly-generated throwaway key → signature won't match JWKS
        from cryptography.hazmat.primitives.asymmetric import rsa as _rsa
        bad_key = _rsa.generate_private_key(public_exponent=65537, key_size=2048)
        private_pem = bad_key.private_bytes(
            serialization.Encoding.PEM,
            serialization.PrivateFormat.TraditionalOpenSSL,
            serialization.NoEncryption(),
        )
    else:
        private_pem = _oidc_private_key.private_bytes(
            serialization.Encoding.PEM,
            serialization.PrivateFormat.TraditionalOpenSSL,
            serialization.NoEncryption(),
        )

    token = pyjwt.encode(payload, private_pem, algorithm="RS256", headers=headers)
    return {"token": token, "expires_at": exp, "issuer": MOCK_BASE_URL}


# ─────────────────────────────────────────────────────────────────────────────
# Echo endpoint — returns the raw request headers + body for verification
# Used by integration tests to confirm transforms, header injection, etc.
# ─────────────────────────────────────────────────────────────────────────────

@app.post("/echo")
@app.get("/echo")
async def echo_endpoint(request: Request):
    """Returns the exact headers and body the mock received.
    This lets tests verify that transforms, header injection, and body
    field modifications actually reached the upstream."""
    try:
        body = await request.json()
    except Exception:
        body = None
    return JSONResponse({
        "headers": dict(request.headers),
        "body": body,
        "method": request.method,
        "url": str(request.url),
        "query_params": dict(request.query_params),
    })


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=9000, log_level="info")

