use serde_json::{json, Value};

use super::bedrock::translate_bedrock_event_stream_to_openai;
use super::Provider;

#[allow(dead_code)]
pub(crate) fn translate_sse_body(provider: Provider, body: &[u8], model: &str) -> Option<Vec<u8>> {
    match provider {
        Provider::Anthropic => Some(translate_anthropic_sse_to_openai(body, model)),
        Provider::Gemini => Some(translate_gemini_sse_to_openai(body, model)),
        // Bedrock uses binary event stream (application/vnd.amazon.eventstream),
        // NOT SSE. Its streaming is handled separately in the handler via
        // decode_bedrock_event_stream() + translate_bedrock_stream_to_openai_sse().
        // Returning None here means the SSE passthrough path is used,
        // but for Bedrock the handler intercepts before reaching this.
        Provider::Bedrock => Some(translate_bedrock_event_stream_to_openai(body, model)),
        // OpenAI-compatible providers — no SSE translation needed
        Provider::OpenAI
        | Provider::AzureOpenAI
        | Provider::Groq
        | Provider::Mistral
        | Provider::TogetherAI
        | Provider::Cohere
        | Provider::Ollama
        | Provider::Unknown => None,
    }
}

/// Generate an OpenAI-format SSE chunk line.
pub(crate) fn openai_sse_chunk(
    chunk_id: &str,
    model: &str,
    delta: Value,
    finish_reason: Option<&str>,
) -> String {
    let chunk = json!({
        "id": chunk_id,
        "object": "chat.completion.chunk",
        "created": chrono::Utc::now().timestamp(),
        "model": model,
        "choices": [{
            "index": 0,
            "delta": delta,
            "finish_reason": finish_reason,
        }]
    });
    format!(
        "data: {}\n\n",
        serde_json::to_string(&chunk).unwrap_or_default()
    )
}

// ── Anthropic SSE → OpenAI SSE ──────────────────────────────────

pub(crate) fn translate_anthropic_sse_to_openai(body: &[u8], model: &str) -> Vec<u8> {
    let body_str = String::from_utf8_lossy(body);
    let chunk_id = format!("chatcmpl-{}", uuid::Uuid::new_v4().simple());
    let mut output = String::new();
    let mut sent_role = false;

    // FIX #3: Track usage tokens from Anthropic streaming events.
    // Anthropic sends input_tokens in message_start and output_tokens in message_delta.
    let mut input_tokens: Option<u64> = None;
    let mut output_tokens: Option<u64> = None;

    // Anthropic SSE has two relevant line types:
    // `event: <type>` followed by `data: <json>`
    // We track current event type and process data lines.
    let mut current_event_type: Option<String> = None;

    for line in body_str.lines() {
        let line = line.trim();

        if line.is_empty() {
            current_event_type = None;
            continue;
        }

        // Track event type
        if let Some(event_type) = line.strip_prefix("event: ") {
            current_event_type = Some(event_type.trim().to_string());
            continue;
        }

        // Process data lines
        let data = if let Some(stripped) = line.strip_prefix("data: ") {
            stripped.trim()
        } else if let Some(stripped) = line.strip_prefix("data:") {
            stripped.trim()
        } else {
            continue;
        };

        if data == "[DONE]" {
            output.push_str("data: [DONE]\n\n");
            continue;
        }

        let json: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event_type = current_event_type
            .as_deref()
            .or_else(|| json.get("type").and_then(|t| t.as_str()))
            .unwrap_or("");

        match event_type {
            "message_start" => {
                // Emit role chunk
                if !sent_role {
                    output.push_str(&openai_sse_chunk(
                        &chunk_id,
                        model,
                        json!({"role": "assistant", "content": ""}),
                        None,
                    ));
                    sent_role = true;
                }
                // FIX #3: Extract input_tokens from message_start.
                // Anthropic format: {"type":"message_start","message":{"usage":{"input_tokens":N}}}
                if let Some(usage) = json.get("message").and_then(|m| m.get("usage")) {
                    if let Some(inp) = usage.get("input_tokens").and_then(|t| t.as_u64()) {
                        input_tokens = Some(inp);
                    }
                }
            }
            "content_block_delta" => {
                if let Some(delta) = json.get("delta") {
                    // Text delta
                    if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                        output.push_str(&openai_sse_chunk(
                            &chunk_id,
                            model,
                            json!({"content": text}),
                            None,
                        ));
                    }
                    // Tool input delta
                    if let Some(partial) = delta.get("partial_json").and_then(|p| p.as_str()) {
                        let index = json.get("index").and_then(|i| i.as_u64()).unwrap_or(0);
                        output.push_str(&openai_sse_chunk(
                            &chunk_id, model,
                            json!({"tool_calls": [{"index": index, "function": {"arguments": partial}}]}),
                            None,
                        ));
                    }
                }
            }
            "content_block_start" => {
                // Tool use start → emit tool call header
                if let Some(cb) = json.get("content_block") {
                    if cb.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                        let index = json.get("index").and_then(|i| i.as_u64()).unwrap_or(0);
                        let name = cb.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        let call_id = cb.get("id").and_then(|id| id.as_str()).unwrap_or("");
                        output.push_str(&openai_sse_chunk(
                            &chunk_id,
                            model,
                            json!({"tool_calls": [{
                                "index": index,
                                "id": call_id,
                                "type": "function",
                                "function": {"name": name, "arguments": ""}
                            }]}),
                            None,
                        ));
                    }
                }
            }
            "message_delta" => {
                // FIX #3: Extract output_tokens from message_delta.
                // Anthropic format: {"type":"message_delta","usage":{"output_tokens":N}}
                if let Some(usage) = json.get("usage") {
                    if let Some(out) = usage.get("output_tokens").and_then(|t| t.as_u64()) {
                        output_tokens = Some(out);
                    }
                }

                // Map stop_reason → finish_reason
                let stop = json
                    .get("delta")
                    .and_then(|d| d.get("stop_reason"))
                    .and_then(|s| s.as_str());
                let finish = match stop {
                    Some("end_turn") => Some("stop"),
                    Some("tool_use") => Some("tool_calls"),
                    Some("max_tokens") => Some("length"),
                    Some("stop_sequence") => Some("stop"),
                    // FIX H-2: Default to "stop" for unknown stop reasons so usage
                    // is still emitted in the final chunk instead of being silently dropped.
                    Some(_) => Some("stop"),
                    None => None,
                };
                if let Some(fr) = finish {
                    // FIX #3: Emit usage in the final chunk (matches OpenAI stream_options behavior).
                    // OpenAI includes usage in the last chunk when stream_options.include_usage is set.
                    let prompt = input_tokens.unwrap_or(0);
                    let completion = output_tokens.unwrap_or(0);
                    let chunk = json!({
                        "id": chunk_id,
                        "object": "chat.completion.chunk",
                        "created": chrono::Utc::now().timestamp(),
                        "model": model,
                        "choices": [{
                            "index": 0,
                            "delta": {},
                            "finish_reason": fr,
                        }],
                        "usage": {
                            "prompt_tokens": prompt,
                            "completion_tokens": completion,
                            "total_tokens": prompt + completion,
                        }
                    });
                    output.push_str(&format!(
                        "data: {}\n\n",
                        serde_json::to_string(&chunk).unwrap_or_default()
                    ));
                }
            }
            "message_stop" => {
                output.push_str("data: [DONE]\n\n");
            }
            _ => {}
        }
    }

    output.into_bytes()
}

// ── Gemini SSE → OpenAI SSE ─────────────────────────────────────

pub(crate) fn translate_gemini_sse_to_openai(body: &[u8], model: &str) -> Vec<u8> {
    let body_str = String::from_utf8_lossy(body);
    let chunk_id = format!("chatcmpl-{}", uuid::Uuid::new_v4().simple());
    let mut output = String::new();
    let mut sent_role = false;

    // Track usage from usageMetadata (Gemini includes this in each chunk;
    // we keep the latest values and emit them in the final chunk).
    let mut prompt_tokens: Option<u64> = None;
    let mut completion_tokens: Option<u64> = None;

    for line in body_str.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let data = if let Some(stripped) = line.strip_prefix("data: ") {
            stripped.trim()
        } else if let Some(stripped) = line.strip_prefix("data:") {
            stripped.trim()
        } else {
            continue;
        };

        if data == "[DONE]" {
            output.push_str("data: [DONE]\n\n");
            continue;
        }

        let json: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Emit role on first chunk
        if !sent_role {
            output.push_str(&openai_sse_chunk(
                &chunk_id,
                model,
                json!({"role": "assistant", "content": ""}),
                None,
            ));
            sent_role = true;
        }

        // Extract usage from usageMetadata (present in each chunk, use latest)
        if let Some(usage_meta) = json.get("usageMetadata") {
            if let Some(pt) = usage_meta.get("promptTokenCount").and_then(|v| v.as_u64()) {
                prompt_tokens = Some(pt);
            }
            if let Some(ct) = usage_meta
                .get("candidatesTokenCount")
                .and_then(|v| v.as_u64())
            {
                completion_tokens = Some(ct);
            }
        }

        // Extract text from candidates[0].content.parts
        if let Some(candidates) = json.get("candidates").and_then(|c| c.as_array()) {
            if let Some(candidate) = candidates.first() {
                let parts = candidate
                    .get("content")
                    .and_then(|c| c.get("parts"))
                    .and_then(|p| p.as_array());

                if let Some(parts) = parts {
                    for part in parts {
                        // Text part
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            output.push_str(&openai_sse_chunk(
                                &chunk_id,
                                model,
                                json!({"content": text}),
                                None,
                            ));
                        }
                        // Function call part
                        if let Some(fc) = part.get("functionCall") {
                            let name = fc.get("name").and_then(|n| n.as_str()).unwrap_or("");
                            let args = fc
                                .get("args")
                                .map(|v| serde_json::to_string(v).unwrap_or_default())
                                .unwrap_or_default();
                            output.push_str(&openai_sse_chunk(
                                &chunk_id,
                                model,
                                json!({"tool_calls": [{
                                    "index": 0,
                                    "id": format!("call_{}", uuid::Uuid::new_v4().simple()),
                                    "type": "function",
                                    "function": {"name": name, "arguments": args}
                                }]}),
                                None,
                            ));
                        }
                    }
                }

                // Check finish reason — emit usage in the final chunk
                let finish = match candidate.get("finishReason").and_then(|f| f.as_str()) {
                    Some("STOP") => Some("stop"),
                    Some("MAX_TOKENS") => Some("length"),
                    Some("SAFETY") => Some("content_filter"),
                    // FIX H-2: Default to "stop" for unknown finish reasons so usage
                    // is still emitted instead of being silently dropped.
                    Some(_) => Some("stop"),
                    None => None,
                };
                if let Some(fr) = finish {
                    let pt = prompt_tokens.unwrap_or(0);
                    let ct = completion_tokens.unwrap_or(0);
                    let chunk = json!({
                        "id": chunk_id,
                        "object": "chat.completion.chunk",
                        "created": chrono::Utc::now().timestamp(),
                        "model": model,
                        "choices": [{
                            "index": 0,
                            "delta": {},
                            "finish_reason": fr,
                        }],
                        "usage": {
                            "prompt_tokens": pt,
                            "completion_tokens": ct,
                            "total_tokens": pt + ct,
                        }
                    });
                    output.push_str(&format!(
                        "data: {}\n\n",
                        serde_json::to_string(&chunk).unwrap_or_default()
                    ));
                }
            }
        }
    }

    // Ensure [DONE] marker
    if !output.ends_with("data: [DONE]\n\n") {
        output.push_str("data: [DONE]\n\n");
    }

    output.into_bytes()
}
