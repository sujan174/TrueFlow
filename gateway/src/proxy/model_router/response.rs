use serde_json::{json, Value};

use super::Provider;

pub(crate) fn translate_response(provider: Provider, body: &Value, model: &str) -> Option<Value> {
    match provider {
        Provider::Anthropic => Some(anthropic_to_openai_response(body, model)),
        Provider::Gemini => Some(gemini_to_openai_response(body, model)),
        Provider::Bedrock => Some(bedrock_to_openai_response(body, model)),
        // OpenAI-compatible providers — no translation needed
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

/// Rewrite the upstream URL for the given provider and model.
///
/// For Azure OpenAI, the URL format is:
///   {endpoint}/openai/deployments/{deployment}/chat/completions?api-version=2024-05-01-preview
///
/// The `base_url` for Azure should be set to the endpoint root, e.g.:
///   https://my-resource.openai.azure.com
/// The `model` field should be the deployment name.
///
/// `is_streaming` is used to select the correct Gemini endpoint:
///   - false → `:generateContent`
///   - true  → `:streamGenerateContent`
pub(crate) fn anthropic_to_openai_response(body: &Value, model: &str) -> Value {
    let content_text = body
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|block| block.get("type").and_then(|t| t.as_str()) == Some("text"))
        })
        .and_then(|block| block.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    // Extract tool calls
    let tool_calls: Vec<Value> = body
        .get("content")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|block| block.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
                .map(|block| {
                    json!({
                        "id": block.get("id").cloned().unwrap_or(json!("")),
                        "type": "function",
                        "function": {
                            "name": block.get("name").cloned().unwrap_or(json!("")),
                            "arguments": block.get("input")
                                .map(|v| serde_json::to_string(v).unwrap_or_default())
                                .unwrap_or_default()
                        }
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let finish_reason = match body.get("stop_reason").and_then(|s| s.as_str()) {
        Some("end_turn") => "stop",
        Some("tool_use") => "tool_calls",
        Some("max_tokens") => "length",
        Some("stop_sequence") => "stop",
        _ => "stop",
    };

    let mut message = json!({
        "role": "assistant",
        "content": content_text,
    });
    if !tool_calls.is_empty() {
        message["tool_calls"] = json!(tool_calls);
    }

    let input_tokens = body
        .get("usage")
        .and_then(|u| u.get("input_tokens"))
        .and_then(|t| t.as_u64())
        .unwrap_or_else(|| {
            // FIX C-2: Use 0 as fallback — we cannot estimate input tokens from
            // the response text. Previously used content_text.len()/4 which was
            // the output length, not input. Zero is safer than a wrong number.
            tracing::warn!(
                model = %model,
                provider = "anthropic",
                "Usage field missing input_tokens — reporting 0 (cannot estimate from response)"
            );
            0
        });
    let output_tokens = body
        .get("usage")
        .and_then(|u| u.get("output_tokens"))
        .and_then(|t| t.as_u64())
        .unwrap_or_else(|| {
            let estimate = (content_text.len() / 4).max(1) as u64;
            tracing::warn!(
                model = %model,
                provider = "anthropic",
                estimated_output_tokens = estimate,
                "Usage field missing output_tokens, using response length estimate"
            );
            estimate
        });

    json!({
        "id": body.get("id").cloned().unwrap_or(json!("msg_unknown")),
        "object": "chat.completion",
        "created": chrono::Utc::now().timestamp(),
        "model": model,
        "choices": [{
            "index": 0,
            "message": message,
            "finish_reason": finish_reason
        }],
        "usage": {
            "prompt_tokens": input_tokens,
            "completion_tokens": output_tokens,
            "total_tokens": input_tokens + output_tokens
        }
    })
}

// ═══════════════════════════════════════════════════════════════
// OpenAI → Gemini (generateContent API)
// ═══════════════════════════════════════════════════════════════

/// Translate an OpenAI content value (string or parts array) into Gemini `parts`.
/// Handles text, image_url (HTTP URLs → fileData, base64 data URIs → inlineData).
pub(crate) fn gemini_to_openai_response(body: &Value, model: &str) -> Value {
    // Extract text from candidates[0].content.parts[0].text
    let candidates = body.get("candidates").and_then(|c| c.as_array());

    let (content_text, finish_reason, tool_calls) =
        if let Some(candidates) = candidates {
            if let Some(candidate) = candidates.first() {
                let text = candidate
                    .get("content")
                    .and_then(|c| c.get("parts"))
                    .and_then(|p| p.as_array())
                    .and_then(|parts| parts.iter().find(|p| p.get("text").is_some()))
                    .and_then(|p| p.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");

                // Extract tool calls from function_call parts
                let tools: Vec<Value> =
                    candidate
                        .get("content")
                        .and_then(|c| c.get("parts"))
                        .and_then(|p| p.as_array())
                        .map(|parts| {
                            parts.iter()
                        .filter_map(|p| p.get("functionCall"))
                        .enumerate()
                        .map(|(i, fc)| json!({
                            "id": format!("call_{}", i),
                            "type": "function",
                            "function": {
                                "name": fc.get("name").cloned().unwrap_or(json!("")),
                                "arguments": fc.get("args")
                                    .map(|v| serde_json::to_string(v).unwrap_or_default())
                                    .unwrap_or_default()
                            }
                        }))
                        .collect()
                        })
                        .unwrap_or_default();

                let reason = match candidate.get("finishReason").and_then(|f| f.as_str()) {
                    Some("STOP") => "stop",
                    Some("MAX_TOKENS") => "length",
                    Some("SAFETY") => "content_filter",
                    Some("RECITATION") => "content_filter",
                    _ => "stop",
                };

                (text.to_string(), reason, tools)
            } else {
                (String::new(), "stop", Vec::new())
            }
        } else {
            (String::new(), "stop", Vec::new())
        };

    let mut message = json!({
        "role": "assistant",
        "content": content_text,
    });
    if !tool_calls.is_empty() {
        message["tool_calls"] = json!(tool_calls);
    }

    let prompt_tokens = body
        .get("usageMetadata")
        .and_then(|u| u.get("promptTokenCount"))
        .and_then(|t| t.as_u64())
        .unwrap_or_else(|| {
            // FIX: Use 0 as fallback — we cannot estimate input tokens from
            // the response text. Using content_text.len()/4 estimates from
            // OUTPUT length, not input. Zero is safer than a wrong number.
            tracing::warn!(
                model = %model,
                provider = "gemini",
                "usageMetadata missing promptTokenCount — reporting 0 (cannot estimate from response)"
            );
            0
        });
    let completion_tokens = body
        .get("usageMetadata")
        .and_then(|u| u.get("candidatesTokenCount"))
        .and_then(|t| t.as_u64())
        .unwrap_or_else(|| {
            let estimate = (content_text.len() / 4).max(1) as u64;
            tracing::warn!(
                model = %model,
                provider = "gemini",
                estimated_completion_tokens = estimate,
                "usageMetadata missing candidatesTokenCount, using conservative estimate"
            );
            estimate
        });

    json!({
        "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        "object": "chat.completion",
        "created": chrono::Utc::now().timestamp(),
        "model": model,
        "choices": [{
            "index": 0,
            "message": message,
            "finish_reason": finish_reason
        }],
        "usage": {
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": prompt_tokens + completion_tokens
        }
    })
}

// ═══════════════════════════════════════════════════════════════
// SSE Stream Translation (Anthropic/Gemini → OpenAI delta format)
// ═══════════════════════════════════════════════════════════════

/// Translate an entire SSE response body from a non-OpenAI provider
/// into OpenAI-compatible `chat.completion.chunk` SSE events.
/// Returns `None` if no translation is needed (OpenAI/Unknown).
#[allow(dead_code)]
pub(crate) fn bedrock_to_openai_response(body: &Value, model: &str) -> Value {
    // Extract content from output.message.content[]
    let message = body.get("output").and_then(|o| o.get("message"));

    let (content_text, tool_calls) = if let Some(msg) = message {
        let content_blocks = msg.get("content").and_then(|c| c.as_array());

        let text: String = content_blocks
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        let tools: Vec<Value> = content_blocks
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|b| b.get("toolUse"))
                    .map(|tu| {
                        json!({
                            "id": tu.get("toolUseId").cloned().unwrap_or(json!("")),
                            "type": "function",
                            "function": {
                                "name": tu.get("name").cloned().unwrap_or(json!("")),
                                "arguments": tu.get("input")
                                    .map(|v| serde_json::to_string(v).unwrap_or_default())
                                    .unwrap_or_default()
                            }
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        (text, tools)
    } else {
        (String::new(), Vec::new())
    };

    let finish_reason = match body.get("stopReason").and_then(|s| s.as_str()) {
        Some("end_turn") => "stop",
        Some("tool_use") => "tool_calls",
        Some("max_tokens") => "length",
        Some("stop_sequence") => "stop",
        Some("content_filtered") => "content_filter",
        Some("guardrail_intervened") => "content_filter",
        _ => "stop",
    };

    let mut oai_message = json!({
        "role": "assistant",
        "content": content_text,
    });
    if !tool_calls.is_empty() {
        oai_message["tool_calls"] = json!(tool_calls);
    }

    let input_tokens = body
        .get("usage")
        .and_then(|u| u.get("inputTokens"))
        .and_then(|t| t.as_u64())
        .unwrap_or(0);
    let output_tokens = body
        .get("usage")
        .and_then(|u| u.get("outputTokens"))
        .and_then(|t| t.as_u64())
        .unwrap_or(0);

    json!({
        "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        "object": "chat.completion",
        "created": chrono::Utc::now().timestamp(),
        "model": model,
        "choices": [{
            "index": 0,
            "message": oai_message,
            "finish_reason": finish_reason
        }],
        "usage": {
            "prompt_tokens": input_tokens,
            "completion_tokens": output_tokens,
            "total_tokens": input_tokens + output_tokens
        }
    })
}
