use super::bedrock::*;
use super::error::*;
use super::headers::*;
use super::request::*;
use super::response::*;
use super::streaming::*;
use super::url_rewrite::*;
use super::*;
use serde_json::json;

// ── Provider Detection ──────────────────────────────────────

#[test]
fn test_detect_openai_models() {
    assert_eq!(detect_provider("gpt-4", ""), Provider::OpenAI);
    assert_eq!(detect_provider("gpt-4o-mini", ""), Provider::OpenAI);
    assert_eq!(detect_provider("o1-preview", ""), Provider::OpenAI);
    assert_eq!(detect_provider("o3-mini", ""), Provider::OpenAI);
}

#[test]
fn test_detect_anthropic_models() {
    assert_eq!(detect_provider("claude-3-opus", ""), Provider::Anthropic);
    assert_eq!(
        detect_provider("claude-3.5-sonnet", ""),
        Provider::Anthropic
    );
    assert_eq!(
        detect_provider("claude-instant-1.2", ""),
        Provider::Anthropic
    );
}

#[test]
fn test_detect_gemini_models() {
    assert_eq!(detect_provider("gemini-2.0-flash", ""), Provider::Gemini);
    assert_eq!(detect_provider("gemini-pro", ""), Provider::Gemini);
}

#[test]
fn test_detect_from_url_fallback() {
    assert_eq!(
        detect_provider("custom-model", "https://api.anthropic.com"),
        Provider::Anthropic
    );
    assert_eq!(
        detect_provider("custom-model", "https://generativelanguage.googleapis.com"),
        Provider::Gemini
    );
    assert_eq!(
        detect_provider("custom-model", "https://api.openai.com"),
        Provider::OpenAI
    );
}

#[test]
fn test_detect_unknown() {
    assert_eq!(
        detect_provider("llama-3", "https://custom.local"),
        Provider::Unknown
    );
}

// ── OpenAI → Anthropic Translation ──────────────────────────

#[test]
fn test_openai_to_anthropic_basic() {
    let body = json!({
        "model": "claude-3-opus-20240229",
        "messages": [
            {"role": "system", "content": "You are helpful."},
            {"role": "user", "content": "Hello!"}
        ],
        "temperature": 0.7,
        "max_tokens": 1024
    });

    let translated = openai_to_anthropic_request(&body);

    assert_eq!(translated["model"], "claude-3-opus-20240229");
    assert_eq!(translated["max_tokens"], 1024);
    assert_eq!(translated["system"], "You are helpful.");
    assert_eq!(translated["temperature"], 0.7);

    let messages = translated["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1); // system extracted
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[0]["content"], "Hello!");
}

#[test]
fn test_openai_to_anthropic_with_tools() {
    let body = json!({
        "model": "claude-3-opus-20240229",
        "messages": [{"role": "user", "content": "What's the weather?"}],
        "tools": [{
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "Get the weather",
                "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}
            }
        }]
    });

    let translated = openai_to_anthropic_request(&body);
    let tools = translated["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "get_weather");
    assert!(tools[0].get("input_schema").is_some());
}

#[test]
fn test_anthropic_to_openai_response() {
    let body = json!({
        "id": "msg_01abc",
        "type": "message",
        "content": [{"type": "text", "text": "Hello!"}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 10, "output_tokens": 5}
    });

    let translated = anthropic_to_openai_response(&body, "claude-3-opus");
    assert_eq!(translated["id"], "msg_01abc");
    assert_eq!(translated["object"], "chat.completion");
    assert_eq!(translated["choices"][0]["message"]["content"], "Hello!");
    assert_eq!(translated["choices"][0]["finish_reason"], "stop");
    assert_eq!(translated["usage"]["prompt_tokens"], 10);
    assert_eq!(translated["usage"]["completion_tokens"], 5);
    assert_eq!(translated["usage"]["total_tokens"], 15);
}

#[test]
fn test_anthropic_tool_use_response() {
    let body = json!({
        "id": "msg_01abc",
        "content": [
            {"type": "text", "text": "Let me check."},
            {"type": "tool_use", "id": "toolu_01", "name": "get_weather", "input": {"city": "NYC"}}
        ],
        "stop_reason": "tool_use",
        "usage": {"input_tokens": 10, "output_tokens": 20}
    });

    let translated = anthropic_to_openai_response(&body, "claude-3-opus");
    assert_eq!(translated["choices"][0]["finish_reason"], "tool_calls");
    let tool_calls = translated["choices"][0]["message"]["tool_calls"]
        .as_array()
        .unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0]["function"]["name"], "get_weather");
}

// ── OpenAI → Gemini Translation ─────────────────────────────

#[test]
fn test_openai_to_gemini_basic() {
    let body = json!({
        "model": "gemini-2.0-flash",
        "messages": [
            {"role": "system", "content": "You are helpful."},
            {"role": "user", "content": "Hello!"},
            {"role": "assistant", "content": "Hi!"},
            {"role": "user", "content": "How are you?"}
        ],
        "temperature": 0.5,
        "max_tokens": 512
    });

    let translated = openai_to_gemini_request(&body);

    // System instruction should be extracted
    assert!(translated.get("systemInstruction").is_some());
    assert_eq!(
        translated["systemInstruction"]["parts"][0]["text"],
        "You are helpful."
    );

    // Contents should have user/model roles
    let contents = translated["contents"].as_array().unwrap();
    assert_eq!(contents.len(), 3); // system excluded
    assert_eq!(contents[0]["role"], "user");
    assert_eq!(contents[1]["role"], "model"); // assistant → model

    // Generation config
    assert_eq!(translated["generationConfig"]["temperature"], 0.5);
    assert_eq!(translated["generationConfig"]["maxOutputTokens"], 512);
}

#[test]
fn test_gemini_to_openai_response() {
    let body = json!({
        "candidates": [{
            "content": {
                "parts": [{"text": "Hello!"}],
                "role": "model"
            },
            "finishReason": "STOP"
        }],
        "usageMetadata": {
            "promptTokenCount": 8,
            "candidatesTokenCount": 3,
            "totalTokenCount": 11
        }
    });

    let translated = gemini_to_openai_response(&body, "gemini-2.0-flash");
    assert_eq!(translated["object"], "chat.completion");
    assert_eq!(translated["choices"][0]["message"]["content"], "Hello!");
    assert_eq!(translated["choices"][0]["finish_reason"], "stop");
    assert_eq!(translated["usage"]["prompt_tokens"], 8);
    assert_eq!(translated["usage"]["completion_tokens"], 3);
}

#[test]
fn test_gemini_tool_call_response() {
    let body = json!({
        "candidates": [{
            "content": {
                "parts": [
                    {"functionCall": {"name": "get_weather", "args": {"city": "NYC"}}}
                ],
                "role": "model"
            },
            "finishReason": "STOP"
        }],
        "usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 5}
    });

    let translated = gemini_to_openai_response(&body, "gemini-2.0-flash");
    let tool_calls = translated["choices"][0]["message"]["tool_calls"]
        .as_array()
        .unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0]["function"]["name"], "get_weather");
}

// ── URL Rewriting ───────────────────────────────────────────

#[test]
fn test_rewrite_gemini_url_non_streaming() {
    let url = rewrite_upstream_url(
        Provider::Gemini,
        "https://generativelanguage.googleapis.com",
        "gemini-2.0-flash",
        false,
    );
    assert_eq!(
        url,
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent"
    );
}

#[test]
fn test_rewrite_gemini_url_streaming() {
    let url = rewrite_upstream_url(
        Provider::Gemini,
        "https://generativelanguage.googleapis.com",
        "gemini-2.0-flash",
        true,
    );
    assert_eq!(url, "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:streamGenerateContent");
}

#[test]
fn test_rewrite_anthropic_url() {
    let url = rewrite_upstream_url(
        Provider::Anthropic,
        "https://api.anthropic.com",
        "claude-3-opus",
        false,
    );
    assert_eq!(url, "https://api.anthropic.com/v1/messages");
}

#[test]
fn test_rewrite_openai_url_passthrough() {
    let url = rewrite_upstream_url(
        Provider::OpenAI,
        "https://api.openai.com/v1/chat/completions",
        "gpt-4",
        false,
    );
    assert_eq!(url, "https://api.openai.com/v1/chat/completions");
}

// ── Multimodal Content (Gemini) ─────────────────────────────

#[test]
fn test_gemini_multimodal_base64_image() {
    let body = json!({
        "model": "gemini-2.0-flash",
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": "What is this?"},
                {"type": "image_url", "image_url": {"url": "data:image/png;base64,iVBORw0K"}}
            ]
        }]
    });
    let translated = openai_to_gemini_request(&body);
    let parts = &translated["contents"][0]["parts"];
    assert!(parts.as_array().unwrap().len() == 2);
    // First part: text
    assert_eq!(parts[0]["text"], "What is this?");
    // Second part: inlineData (base64)
    assert!(parts[1].get("inlineData").is_some());
    assert_eq!(parts[1]["inlineData"]["mimeType"], "image/png");
    assert_eq!(parts[1]["inlineData"]["data"], "iVBORw0K");
}

#[test]
fn test_gemini_multimodal_url_image() {
    let body = json!({
        "model": "gemini-2.0-flash",
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": "Describe:"},
                {"type": "image_url", "image_url": {"url": "https://example.com/photo.png"}}
            ]
        }]
    });
    let translated = openai_to_gemini_request(&body);
    let parts = &translated["contents"][0]["parts"];
    // Second part: fileData (HTTP URL)
    assert!(parts[1].get("fileData").is_some());
    assert_eq!(parts[1]["fileData"]["mimeType"], "image/png");
    assert_eq!(
        parts[1]["fileData"]["fileUri"],
        "https://example.com/photo.png"
    );
}

// ── response_format (Gemini) ────────────────────────────────

#[test]
fn test_gemini_response_format_json_object() {
    let body = json!({
        "model": "gemini-2.0-flash",
        "messages": [{"role": "user", "content": "Return JSON"}],
        "response_format": {"type": "json_object"}
    });
    let translated = openai_to_gemini_request(&body);
    assert_eq!(
        translated["generationConfig"]["responseMimeType"],
        "application/json"
    );
}

#[test]
fn test_gemini_response_format_json_schema() {
    let body = json!({
        "model": "gemini-2.0-flash",
        "messages": [{"role": "user", "content": "Return structured JSON"}],
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "my_schema",
                "schema": {"type": "object", "properties": {"name": {"type": "string"}}}
            }
        }
    });
    let translated = openai_to_gemini_request(&body);
    assert_eq!(
        translated["generationConfig"]["responseMimeType"],
        "application/json"
    );
    assert!(translated["generationConfig"]
        .get("responseSchema")
        .is_some());
}

// ── tool_choice (Anthropic) ─────────────────────────────────

#[test]
fn test_anthropic_tool_choice_auto() {
    let body = json!({
        "model": "claude-3-opus",
        "messages": [{"role": "user", "content": "hi"}],
        "tools": [{"type": "function", "function": {"name": "foo", "description": "", "parameters": {}}}],
        "tool_choice": "auto"
    });
    let translated = openai_to_anthropic_request(&body);
    assert_eq!(translated["tool_choice"]["type"], "auto");
}

#[test]
fn test_anthropic_tool_choice_required() {
    let body = json!({
        "model": "claude-3-opus",
        "messages": [{"role": "user", "content": "hi"}],
        "tools": [{"type": "function", "function": {"name": "foo", "description": "", "parameters": {}}}],
        "tool_choice": "required"
    });
    let translated = openai_to_anthropic_request(&body);
    assert_eq!(translated["tool_choice"]["type"], "any");
}

#[test]
fn test_anthropic_tool_choice_specific_function() {
    let body = json!({
        "model": "claude-3-opus",
        "messages": [{"role": "user", "content": "hi"}],
        "tools": [{"type": "function", "function": {"name": "get_weather", "description": "", "parameters": {}}}],
        "tool_choice": {"type": "function", "function": {"name": "get_weather"}}
    });
    let translated = openai_to_anthropic_request(&body);
    assert_eq!(translated["tool_choice"]["type"], "tool");
    assert_eq!(translated["tool_choice"]["name"], "get_weather");
}

// ── tool_choice (Gemini) ────────────────────────────────────

#[test]
fn test_gemini_tool_choice_auto() {
    let body = json!({
        "model": "gemini-2.0-flash",
        "messages": [{"role": "user", "content": "hi"}],
        "tool_choice": "auto"
    });
    let translated = openai_to_gemini_request(&body);
    assert_eq!(
        translated["toolConfig"]["functionCallingConfig"]["mode"],
        "AUTO"
    );
}

#[test]
fn test_gemini_tool_choice_none() {
    let body = json!({
        "model": "gemini-2.0-flash",
        "messages": [{"role": "user", "content": "hi"}],
        "tool_choice": "none"
    });
    let translated = openai_to_gemini_request(&body);
    assert_eq!(
        translated["toolConfig"]["functionCallingConfig"]["mode"],
        "NONE"
    );
}

#[test]
fn test_gemini_tool_choice_specific_function() {
    let body = json!({
        "model": "gemini-2.0-flash",
        "messages": [{"role": "user", "content": "hi"}],
        "tool_choice": {"type": "function", "function": {"name": "get_weather"}}
    });
    let translated = openai_to_gemini_request(&body);
    let fc = &translated["toolConfig"]["functionCallingConfig"];
    assert_eq!(fc["mode"], "ANY");
    assert!(fc["allowedFunctionNames"][0] == "get_weather");
}

// ── Provider Header Injection ───────────────────────────────

#[test]
fn test_inject_anthropic_version_header() {
    let mut headers = reqwest::header::HeaderMap::new();
    inject_provider_headers(Provider::Anthropic, &mut headers, false);
    assert_eq!(
        headers
            .get("anthropic-version")
            .and_then(|v| v.to_str().ok()),
        Some("2023-06-01")
    );
}

#[test]
fn test_inject_anthropic_streaming_accept_header() {
    let mut headers = reqwest::header::HeaderMap::new();
    inject_provider_headers(Provider::Anthropic, &mut headers, true);
    assert_eq!(
        headers
            .get("anthropic-version")
            .and_then(|v| v.to_str().ok()),
        Some("2023-06-01")
    );
    assert!(headers.contains_key(reqwest::header::ACCEPT));
}

#[test]
fn test_inject_openai_no_extra_headers() {
    let mut headers = reqwest::header::HeaderMap::new();
    inject_provider_headers(Provider::OpenAI, &mut headers, false);
    assert!(headers.is_empty(), "OpenAI should not inject extra headers");
}

#[test]
fn test_policy_header_wins_over_injection() {
    // If anthropic-version is already set (e.g. by policy), it should not be overwritten
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("anthropic-version", "2025-01-01".parse().unwrap());
    inject_provider_headers(Provider::Anthropic, &mut headers, false);
    assert_eq!(
        headers
            .get("anthropic-version")
            .and_then(|v| v.to_str().ok()),
        Some("2025-01-01") // original value preserved
    );
}

// ── translate_request dispatch ──────────────────────────────

#[test]
fn test_translate_request_openai_passthrough() {
    let body = json!({"model": "gpt-4", "messages": []});
    assert!(translate_request(Provider::OpenAI, &body).is_none());
}

#[test]
fn test_translate_request_anthropic() {
    let body = json!({"model": "claude-3-opus", "messages": [{"role": "user", "content": "hi"}]});
    assert!(translate_request(Provider::Anthropic, &body).is_some());
}

#[test]
fn test_translate_request_gemini() {
    let body = json!({"model": "gemini-pro", "messages": [{"role": "user", "content": "hi"}]});
    assert!(translate_request(Provider::Gemini, &body).is_some());
}

// ── SSE Translation Tests ───────────────────────────────────

#[test]
fn test_translate_sse_openai_passthrough() {
    let body = b"data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\ndata: [DONE]\n\n";
    assert!(translate_sse_body(Provider::OpenAI, body, "gpt-4").is_none());
    assert!(translate_sse_body(Provider::Unknown, body, "custom").is_none());
}

#[test]
fn test_anthropic_sse_text_streaming() {
    let body = b"\
event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_01\",\"model\":\"claude-3-opus\",\"usage\":{\"input_tokens\":10}}}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" world\"}}\n\
\n\
event: message_delta\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\
\n\
event: message_stop\n\
data: {\"type\":\"message_stop\"}\n\
\n";

    let result = translate_sse_body(Provider::Anthropic, body, "claude-3-opus");
    assert!(result.is_some());
    let output = String::from_utf8(result.unwrap()).unwrap();

    // Should contain OpenAI-format chunks
    assert!(output.contains("chat.completion.chunk"));
    // Role chunk
    assert!(output.contains("\"role\":\"assistant\""));
    // Text deltas
    assert!(output.contains("\"content\":\"Hello\""));
    assert!(output.contains("\"content\":\" world\""));
    // Finish reason
    assert!(output.contains("\"finish_reason\":\"stop\""));
    // Done marker
    assert!(output.contains("data: [DONE]"));
}

#[test]
fn test_anthropic_sse_tool_streaming() {
    let body = b"\
event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_02\",\"model\":\"claude-3-opus\"}}\n\
\n\
event: content_block_start\n\
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_01\",\"name\":\"get_weather\"}}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"city\\\"\"}}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\": \\\"NYC\\\"}\"}}\n\
\n\
event: message_delta\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"}}\n\
\n\
event: message_stop\n\
data: {\"type\":\"message_stop\"}\n\
\n";

    let result = translate_sse_body(Provider::Anthropic, body, "claude-3-opus");
    let output = String::from_utf8(result.unwrap()).unwrap();

    // Should emit tool call header
    assert!(output.contains("\"name\":\"get_weather\""));
    assert!(output.contains("\"id\":\"toolu_01\""));
    // Tool call argument deltas
    assert!(output.contains("\"arguments\":\"{\\\"city\\\"\""));
    // Finish reason for tool use
    assert!(output.contains("\"finish_reason\":\"tool_calls\""));
}

#[test]
fn test_gemini_sse_text_streaming() {
    let body = b"\
data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Hello\"}],\"role\":\"model\"}}]}\n\
\n\
data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\" there!\"}],\"role\":\"model\"},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":5,\"candidatesTokenCount\":3}}\n\
\n";

    let result = translate_sse_body(Provider::Gemini, body, "gemini-2.0-flash");
    let output = String::from_utf8(result.unwrap()).unwrap();

    assert!(output.contains("chat.completion.chunk"));
    assert!(output.contains("\"role\":\"assistant\""));
    assert!(output.contains("\"content\":\"Hello\""));
    assert!(output.contains("\"content\":\" there!\""));
    assert!(output.contains("\"finish_reason\":\"stop\""));
    assert!(output.contains("data: [DONE]"));
}

#[test]
fn test_gemini_sse_function_call() {
    let body = b"\
data: {\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"get_weather\",\"args\":{\"city\":\"NYC\"}}}],\"role\":\"model\"},\"finishReason\":\"STOP\"}]}\n\
\n";

    let result = translate_sse_body(Provider::Gemini, body, "gemini-2.0-flash");
    let output = String::from_utf8(result.unwrap()).unwrap();

    assert!(output.contains("\"name\":\"get_weather\""));
    assert!(output.contains("\"arguments\""));
    assert!(output.contains("data: [DONE]"));
}

#[test]
fn test_gemini_sse_multiple_tool_calls_stable_ids() {
    // Test that multiple tool calls get unique stable IDs
    let body = b"\
data: {\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"get_weather\",\"args\":{\"city\":\"NYC\"}}},{\"functionCall\":{\"name\":\"get_time\",\"args\":{\"tz\":\"EST\"}}}],\"role\":\"model\"},\"finishReason\":\"STOP\"}]}\n\
\n";

    let result = translate_sse_body(Provider::Gemini, body, "gemini-2.0-flash");
    let output = String::from_utf8(result.unwrap()).unwrap();

    // Should contain both function names
    assert!(output.contains("\"name\":\"get_weather\""));
    assert!(output.contains("\"name\":\"get_time\""));

    // Extract tool call IDs from output
    let tool_call_ids: Vec<&str> = output
        .lines()
        .filter_map(|line| {
            if line.contains("\"id\":\"call_") {
                // Extract the ID from the line
                let start = line.find("\"id\":\"call_").unwrap() + 6;
                let end = line[start..].find('"').unwrap() + start;
                Some(&line[start..end])
            } else {
                None
            }
        })
        .collect();

    // Should have exactly 2 tool call IDs (one for each function call)
    assert_eq!(tool_call_ids.len(), 2, "Should have 2 tool call IDs");

    // The two IDs should be different (each tool call gets its own ID)
    assert_ne!(tool_call_ids[0], tool_call_ids[1], "Tool call IDs should be unique");
}

#[test]
fn test_anthropic_sse_empty_body() {
    let body = b"";
    let result = translate_sse_body(Provider::Anthropic, body, "claude-3-opus");
    assert!(result.is_some());
    let output = String::from_utf8(result.unwrap()).unwrap();
    // Should be empty (no events to translate)
    assert!(output.is_empty() || output.trim().is_empty());
}

// ═══════════════════════════════════════════════════════════════
// New Provider Detection Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_detect_mistral_models() {
    assert_eq!(
        detect_provider("mistral-large-latest", ""),
        Provider::Mistral
    );
    assert_eq!(detect_provider("mistral-small-2409", ""), Provider::Mistral);
    assert_eq!(detect_provider("mixtral-8x7b-32768", ""), Provider::Mistral);
}

#[test]
fn test_detect_cohere_models() {
    assert_eq!(detect_provider("command-r-plus", ""), Provider::Cohere);
    assert_eq!(detect_provider("command-r", ""), Provider::Cohere);
    assert_eq!(detect_provider("command-light", ""), Provider::Cohere);
}

#[test]
fn test_detect_together_models() {
    assert_eq!(
        detect_provider("meta-llama/Llama-3-70b", ""),
        Provider::TogetherAI
    );
    assert_eq!(
        detect_provider("mistralai/Mixtral-8x7B-Instruct-v0.1", ""),
        Provider::TogetherAI
    );
    assert_eq!(
        detect_provider("Qwen/Qwen2-72B-Instruct", ""),
        Provider::TogetherAI
    );
    assert_eq!(
        detect_provider("deepseek/deepseek-coder-33b", ""),
        Provider::TogetherAI
    );
    // Generic slash-separated model IDs
    assert_eq!(
        detect_provider("google/gemma-2-9b-it", ""),
        Provider::TogetherAI
    );
    assert_eq!(
        detect_provider("NousResearch/Hermes-2-Theta-Llama-3-8B", ""),
        Provider::TogetherAI
    );
}

#[test]
fn test_detect_bedrock_models() {
    assert_eq!(
        detect_provider("anthropic.claude-v2", ""),
        Provider::Bedrock
    );
    assert_eq!(
        detect_provider("anthropic.claude-3-sonnet-20240229-v1:0", ""),
        Provider::Bedrock
    );
    assert_eq!(
        detect_provider("meta.llama3-1-70b-instruct-v1:0", ""),
        Provider::Bedrock
    );
    assert_eq!(
        detect_provider("amazon.titan-text-premier-v1:0", ""),
        Provider::Bedrock
    );
    assert_eq!(
        detect_provider("cohere.command-r-plus-v1:0", ""),
        Provider::Bedrock
    );
}

#[test]
fn test_detect_new_providers_from_url() {
    assert_eq!(
        detect_provider("custom", "https://api.groq.com/openai/v1"),
        Provider::Groq
    );
    assert_eq!(
        detect_provider("custom", "https://api.mistral.ai/v1"),
        Provider::Mistral
    );
    assert_eq!(
        detect_provider("custom", "https://api.together.xyz/v1"),
        Provider::TogetherAI
    );
    assert_eq!(
        detect_provider("custom", "https://api.together.ai/v1"),
        Provider::TogetherAI
    );
    assert_eq!(
        detect_provider("custom", "https://api.cohere.com/v1"),
        Provider::Cohere
    );
    assert_eq!(
        detect_provider("custom", "http://localhost:11434"),
        Provider::Ollama
    );
    assert_eq!(
        detect_provider("custom", "https://bedrock-runtime.us-east-1.amazonaws.com"),
        Provider::Bedrock
    );
    assert_eq!(
        detect_provider("custom", "https://my-resource.openai.azure.com"),
        Provider::AzureOpenAI
    );
}

#[test]
fn test_bedrock_url_does_not_match_anthropic() {
    // URLs containing both "bedrock" and "anthropic" should match Bedrock, not Anthropic
    assert_eq!(
        detect_provider(
            "custom",
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/anthropic.claude-v2/converse"
        ),
        Provider::Bedrock
    );
}

// ═══════════════════════════════════════════════════════════════
// URL Rewriting Tests (New Providers)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_rewrite_azure_url_with_api_version() {
    let url = rewrite_upstream_url(
        Provider::AzureOpenAI,
        "https://my-resource.openai.azure.com",
        "gpt-4",
        false,
    );
    assert!(url.contains("/openai/deployments/gpt-4/chat/completions"));
    assert!(url.contains("api-version="));
}

#[test]
fn test_rewrite_azure_url_existing_deployment_path() {
    let url = rewrite_upstream_url(
        Provider::AzureOpenAI,
        "https://my-resource.openai.azure.com/openai/deployments/my-deploy/chat/completions",
        "gpt-4",
        false,
    );
    // Should preserve existing path and add api-version
    assert!(url.contains("api-version="));
}

#[test]
fn test_rewrite_bedrock_url_non_streaming() {
    let url = rewrite_upstream_url(
        Provider::Bedrock,
        "https://bedrock-runtime.us-east-1.amazonaws.com",
        "anthropic.claude-v2",
        false,
    );
    assert_eq!(
        url,
        "https://bedrock-runtime.us-east-1.amazonaws.com/model/anthropic.claude-v2/converse"
    );
}

#[test]
fn test_rewrite_bedrock_url_streaming() {
    let url = rewrite_upstream_url(
        Provider::Bedrock,
        "https://bedrock-runtime.us-east-1.amazonaws.com",
        "anthropic.claude-v2",
        true,
    );
    assert_eq!(
        url,
        "https://bedrock-runtime.us-east-1.amazonaws.com/model/anthropic.claude-v2/converse-stream"
    );
}

#[test]
fn test_rewrite_together_url() {
    let url = rewrite_upstream_url(
        Provider::TogetherAI,
        "https://api.together.xyz",
        "meta-llama/Llama-3-70b",
        false,
    );
    assert_eq!(url, "https://api.together.xyz/v1/chat/completions");
}

#[test]
fn test_rewrite_groq_url() {
    let url = rewrite_upstream_url(
        Provider::Groq,
        "https://api.groq.com/openai/v1/chat/completions",
        "mixtral-8x7b-32768",
        false,
    );
    // Should strip /v1/chat/completions and re-add it
    assert!(url.contains("groq.com"));
    assert!(url.contains("/v1"));
}

// ═══════════════════════════════════════════════════════════════
// Model Name Sanitization Tests (Security)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_sanitize_normal_model_name() {
    // Normal model names should pass through unchanged
    let url = rewrite_upstream_url(
        Provider::Gemini,
        "https://generativelanguage.googleapis.com",
        "gemini-2.0-flash",
        false,
    );
    assert!(url.contains("gemini-2.0-flash"));
}

#[test]
fn test_sanitize_path_traversal_attack() {
    // Path traversal should be rejected (empty model name in URL)
    let url = rewrite_upstream_url(
        Provider::Gemini,
        "https://generativelanguage.googleapis.com",
        "../../../admin/delete",
        false,
    );
    // The model name should be empty (rejected) or encoded
    assert!(!url.contains(".."));
    assert!(!url.contains("admin"));
}

#[test]
fn test_sanitize_query_injection_attack() {
    // Query parameter injection should be encoded
    let url = rewrite_upstream_url(
        Provider::Gemini,
        "https://generativelanguage.googleapis.com",
        "gemini-pro?api_key=stolen",
        false,
    );
    // Should not contain raw ? character that could inject query params
    assert!(!url.contains("?api_key"));
    // The model name should be URL-encoded
    assert!(url.contains("%3F") || url.contains(":"));  // encoded or truncated
}

#[test]
fn test_sanitize_fragment_injection_attack() {
    // Fragment injection should be encoded
    let url = rewrite_upstream_url(
        Provider::Bedrock,
        "https://bedrock-runtime.us-east-1.amazonaws.com",
        "anthropic.claude-v2#fragment",
        false,
    );
    // Should not contain raw # character
    assert!(!url.contains("#fragment"));
}

#[test]
fn test_sanitize_control_characters() {
    // Control characters should be encoded
    let url = rewrite_upstream_url(
        Provider::AzureOpenAI,
        "https://my-resource.openai.azure.com",
        "gpt-4\nmalicious",
        false,
    );
    // Should not contain raw newline
    assert!(!url.contains('\n'));
}

#[test]
fn test_sanitize_whitespace_attack() {
    // Whitespace in model name should be encoded
    let url = rewrite_upstream_url(
        Provider::Gemini,
        "https://generativelanguage.googleapis.com",
        "gemini pro",
        false,
    );
    // Should not contain raw space in URL
    assert!(!url.contains("gemini pro"));
}

#[test]
fn test_sanitize_backslash_attack() {
    // Backslash should be encoded
    let url = rewrite_upstream_url(
        Provider::Bedrock,
        "https://bedrock-runtime.us-east-1.amazonaws.com",
        "anthropic\\claude",
        false,
    );
    // Should not contain raw backslash
    assert!(!url.contains('\\'));
}

// ═══════════════════════════════════════════════════════════════
// Passthrough Verification Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_passthrough_providers_no_request_translation() {
    let body = json!({"model": "test", "messages": [{"role": "user", "content": "hi"}]});
    assert!(translate_request(Provider::Groq, &body).is_none());
    assert!(translate_request(Provider::Mistral, &body).is_none());
    assert!(translate_request(Provider::TogetherAI, &body).is_none());
    assert!(translate_request(Provider::Cohere, &body).is_none());
    assert!(translate_request(Provider::Ollama, &body).is_none());
    assert!(translate_request(Provider::AzureOpenAI, &body).is_none());
}

#[test]
fn test_passthrough_providers_no_response_translation() {
    let body = json!({"choices": [{"message": {"content": "hi"}}]});
    assert!(translate_response(Provider::Groq, &body, "test").is_none());
    assert!(translate_response(Provider::Mistral, &body, "test").is_none());
    assert!(translate_response(Provider::TogetherAI, &body, "test").is_none());
    assert!(translate_response(Provider::Cohere, &body, "test").is_none());
    assert!(translate_response(Provider::Ollama, &body, "test").is_none());
    assert!(translate_response(Provider::AzureOpenAI, &body, "test").is_none());
}

#[test]
fn test_passthrough_providers_no_sse_translation() {
    let body = b"data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n";
    assert!(translate_sse_body(Provider::Groq, body, "test").is_none());
    assert!(translate_sse_body(Provider::Mistral, body, "test").is_none());
    assert!(translate_sse_body(Provider::TogetherAI, body, "test").is_none());
    assert!(translate_sse_body(Provider::Cohere, body, "test").is_none());
    assert!(translate_sse_body(Provider::Ollama, body, "test").is_none());
    assert!(translate_sse_body(Provider::AzureOpenAI, body, "test").is_none());
}

// ═══════════════════════════════════════════════════════════════
// Bedrock Request Translation Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_bedrock_request_basic() {
    let body = json!({
        "model": "anthropic.claude-v2",
        "messages": [
            {"role": "system", "content": "Be helpful."},
            {"role": "user", "content": "Hello!"}
        ],
        "max_tokens": 1024,
        "temperature": 0.7
    });
    let translated = openai_to_bedrock_request(&body);

    assert_eq!(translated["system"][0]["text"], "Be helpful.");
    let msgs = translated["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 1); // system extracted
    assert_eq!(msgs[0]["role"], "user");
    assert_eq!(msgs[0]["content"][0]["text"], "Hello!");
    assert_eq!(translated["inferenceConfig"]["maxTokens"], 1024);
    assert_eq!(translated["inferenceConfig"]["temperature"], 0.7);
}

#[test]
fn test_bedrock_request_with_tools() {
    let body = json!({
        "model": "anthropic.claude-v2",
        "messages": [{"role": "user", "content": "Weather?"}],
        "tools": [{
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "Get the weather",
                "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}
            }
        }]
    });
    let translated = openai_to_bedrock_request(&body);
    let tools = &translated["toolConfig"]["tools"];
    assert_eq!(tools[0]["toolSpec"]["name"], "get_weather");
    assert!(tools[0]["toolSpec"]["inputSchema"]["json"].is_object());
}

#[test]
fn test_bedrock_request_tool_result() {
    let body = json!({
        "model": "anthropic.claude-v2",
        "messages": [
            {"role": "user", "content": "Weather?"},
            {"role": "assistant", "content": "Checking..."},
            {"role": "tool", "tool_call_id": "call_123", "content": "Sunny, 25°C"}
        ]
    });
    let translated = openai_to_bedrock_request(&body);
    let msgs = translated["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 3);
    // Tool result should be wrapped as toolResult
    assert_eq!(msgs[2]["content"][0]["toolResult"]["toolUseId"], "call_123");
    assert_eq!(
        msgs[2]["content"][0]["toolResult"]["content"][0]["text"],
        "Sunny, 25°C"
    );
}

// ═══════════════════════════════════════════════════════════════
// Bedrock Response Translation Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_bedrock_response_text() {
    let body = json!({
        "output": {"message": {"role": "assistant", "content": [{"text": "Hello!"}]}},
        "stopReason": "end_turn",
        "usage": {"inputTokens": 10, "outputTokens": 3}
    });
    let translated = bedrock_to_openai_response(&body, "anthropic.claude-v2");
    assert_eq!(translated["object"], "chat.completion");
    assert_eq!(translated["choices"][0]["message"]["content"], "Hello!");
    assert_eq!(translated["choices"][0]["finish_reason"], "stop");
    assert_eq!(translated["usage"]["prompt_tokens"], 10);
    assert_eq!(translated["usage"]["completion_tokens"], 3);
    assert_eq!(translated["usage"]["total_tokens"], 13);
}

#[test]
fn test_bedrock_response_tool_use() {
    let body = json!({
        "output": {"message": {"role": "assistant", "content": [
            {"text": "Let me check."},
            {"toolUse": {"toolUseId": "call_abc", "name": "get_weather", "input": {"city": "NYC"}}}
        ]}},
        "stopReason": "tool_use",
        "usage": {"inputTokens": 15, "outputTokens": 8}
    });
    let translated = bedrock_to_openai_response(&body, "anthropic.claude-v2");
    assert_eq!(translated["choices"][0]["finish_reason"], "tool_calls");
    let tool_calls = translated["choices"][0]["message"]["tool_calls"]
        .as_array()
        .unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0]["function"]["name"], "get_weather");
    assert_eq!(tool_calls[0]["id"], "call_abc");
}

#[test]
fn test_bedrock_response_max_tokens() {
    let body = json!({
        "output": {"message": {"role": "assistant", "content": [{"text": "truncated..."}]}},
        "stopReason": "max_tokens",
        "usage": {"inputTokens": 10, "outputTokens": 100}
    });
    let translated = bedrock_to_openai_response(&body, "anthropic.claude-v2");
    assert_eq!(translated["choices"][0]["finish_reason"], "length");
}

// ═══════════════════════════════════════════════════════════════
// Bedrock Binary Event Stream Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_bedrock_event_stream_decode() {
    let msg = build_test_bedrock_event(
        "contentBlockDelta",
        json!({
            "contentBlockIndex": 0,
            "delta": {"text": "Hello"}
        }),
    );
    let events = decode_bedrock_event_stream(&msg);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, "contentBlockDelta");
    assert_eq!(events[0].1["delta"]["text"], "Hello");
}

#[test]
fn test_bedrock_event_stream_multiple_frames() {
    let mut stream = Vec::new();
    stream.extend(build_test_bedrock_event(
        "messageStart",
        json!({"role": "assistant"}),
    ));
    stream.extend(build_test_bedrock_event(
        "contentBlockDelta",
        json!({
            "contentBlockIndex": 0, "delta": {"text": "Hello"}
        }),
    ));
    stream.extend(build_test_bedrock_event(
        "contentBlockDelta",
        json!({
            "contentBlockIndex": 0, "delta": {"text": " world"}
        }),
    ));
    stream.extend(build_test_bedrock_event(
        "messageStop",
        json!({"stopReason": "end_turn"}),
    ));
    stream.extend(build_test_bedrock_event(
        "metadata",
        json!({
            "usage": {"inputTokens": 10, "outputTokens": 3}
        }),
    ));

    let events = decode_bedrock_event_stream(&stream);
    assert_eq!(events.len(), 5);
    assert_eq!(events[0].0, "messageStart");
    assert_eq!(events[1].0, "contentBlockDelta");
    assert_eq!(events[3].0, "messageStop");
    assert_eq!(events[4].0, "metadata");
}

#[test]
fn test_bedrock_stream_to_openai_sse() {
    let mut stream = Vec::new();
    stream.extend(build_test_bedrock_event(
        "messageStart",
        json!({"role": "assistant"}),
    ));
    stream.extend(build_test_bedrock_event(
        "contentBlockDelta",
        json!({
            "contentBlockIndex": 0, "delta": {"text": "Hello"}
        }),
    ));
    stream.extend(build_test_bedrock_event(
        "messageStop",
        json!({"stopReason": "end_turn"}),
    ));

    let result = translate_sse_body(Provider::Bedrock, &stream, "anthropic.claude-v2");
    assert!(result.is_some());
    let output = String::from_utf8(result.unwrap()).unwrap();

    assert!(output.contains("chat.completion.chunk"));
    assert!(output.contains("\"role\":\"assistant\""));
    assert!(output.contains("\"content\":\"Hello\""));
    assert!(output.contains("\"finish_reason\":\"stop\""));
    assert!(output.contains("data: [DONE]"));
}

#[test]
fn test_bedrock_stream_tool_use() {
    let mut stream = Vec::new();
    stream.extend(build_test_bedrock_event(
        "messageStart",
        json!({"role": "assistant"}),
    ));
    stream.extend(build_test_bedrock_event(
        "contentBlockStart",
        json!({
            "contentBlockIndex": 0,
            "start": {"toolUse": {"toolUseId": "call_abc", "name": "get_weather"}}
        }),
    ));
    stream.extend(build_test_bedrock_event(
        "messageStop",
        json!({"stopReason": "tool_use"}),
    ));

    let result = translate_sse_body(Provider::Bedrock, &stream, "anthropic.claude-v2");
    let output = String::from_utf8(result.unwrap()).unwrap();

    assert!(output.contains("\"name\":\"get_weather\""));
    assert!(output.contains("\"id\":\"call_abc\""));
    assert!(output.contains("\"finish_reason\":\"tool_calls\""));
}

#[test]
fn test_bedrock_empty_event_stream() {
    let result = translate_sse_body(Provider::Bedrock, b"", "anthropic.claude-v2");
    assert!(result.is_some());
    let output = String::from_utf8(result.unwrap()).unwrap();
    // Should just have the [DONE] marker
    assert!(output.contains("data: [DONE]"));
}

// ═══════════════════════════════════════════════════════════════
// Bedrock Error Normalization Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_bedrock_error_normalization() {
    let body = serde_json::to_vec(&json!({
        "message": "The model is not supported",
        "__type": "ValidationException"
    }))
    .unwrap();
    let normalized = normalize_error_response(Provider::Bedrock, &body);
    assert!(normalized.is_some());
    let err = normalized.unwrap();
    assert_eq!(err["error"]["message"], "The model is not supported");
    assert_eq!(err["error"]["code"], "ValidationException");
    // Type should be snake_case
    assert!(err["error"]["type"]
        .as_str()
        .unwrap()
        .contains("validation"));
}

#[test]
fn test_bedrock_error_normalization_with_hash() {
    let body = serde_json::to_vec(&json!({
        "message": "Access denied",
        "__type": "com.amazonaws.bedrock#AccessDeniedException"
    }))
    .unwrap();
    let normalized = normalize_error_response(Provider::Bedrock, &body);
    assert!(normalized.is_some());
    let err = normalized.unwrap();
    assert_eq!(err["error"]["message"], "Access denied");
    // Should extract the part after # and convert to snake_case
    let err_type = err["error"]["type"].as_str().unwrap();
    assert!(
        err_type.contains("access"),
        "expected 'access' in '{}', got from AccessDenied",
        err_type
    );
}

// ═══════════════════════════════════════════════════════════════
// Provider Header Injection Tests (New Providers)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_groq_streaming_accept_header() {
    let mut headers = reqwest::header::HeaderMap::new();
    inject_provider_headers(Provider::Groq, &mut headers, true);
    assert!(headers.contains_key(reqwest::header::ACCEPT));
}

#[test]
fn test_mistral_streaming_accept_header() {
    let mut headers = reqwest::header::HeaderMap::new();
    inject_provider_headers(Provider::Mistral, &mut headers, true);
    assert!(headers.contains_key(reqwest::header::ACCEPT));
}

#[test]
fn test_together_no_extra_headers_non_streaming() {
    let mut headers = reqwest::header::HeaderMap::new();
    inject_provider_headers(Provider::TogetherAI, &mut headers, false);
    assert!(!headers.contains_key(reqwest::header::ACCEPT));
}

#[test]
fn test_bedrock_streaming_accept_header() {
    let mut headers = reqwest::header::HeaderMap::new();
    inject_provider_headers(Provider::Bedrock, &mut headers, true);
    assert_eq!(
        headers
            .get(reqwest::header::ACCEPT)
            .and_then(|v| v.to_str().ok()),
        Some("application/vnd.amazon.eventstream")
    );
}

#[test]
fn test_bedrock_content_type_header() {
    let mut headers = reqwest::header::HeaderMap::new();
    inject_provider_headers(Provider::Bedrock, &mut headers, false);
    assert!(headers.contains_key(reqwest::header::CONTENT_TYPE));
}
