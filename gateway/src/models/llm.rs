//! Unified LLM response parser.
//!
//! Extracts tool calls, finish reasons, and classifies errors from
//! OpenAI, Anthropic, and Google Gemini response bodies.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Tool Call Extraction ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
}

/// Extract tool calls from a complete (non-streaming) LLM response body.
#[allow(dead_code)]
pub fn extract_tool_calls(body: &[u8]) -> Vec<ToolCallInfo> {
    let json: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    extract_tool_calls_from_value(&json)
}

/// Extract tool calls from an already-parsed JSON value.
pub fn extract_tool_calls_from_value(json: &Value) -> Vec<ToolCallInfo> {
    // Try OpenAI format first (most common)
    let mut results = extract_openai_tool_calls(json);
    if !results.is_empty() {
        return results;
    }

    // Try Anthropic format
    results = extract_anthropic_tool_calls(json);
    if !results.is_empty() {
        return results;
    }

    // Try Gemini format
    extract_gemini_tool_calls(json)
}

/// OpenAI: choices[*].message.tool_calls[*].function.{name, arguments}
fn extract_openai_tool_calls(json: &Value) -> Vec<ToolCallInfo> {
    let mut results = vec![];
    if let Some(choices) = json.get("choices").and_then(|c| c.as_array()) {
        for choice in choices {
            if let Some(tool_calls) = choice
                .get("message")
                .and_then(|m| m.get("tool_calls"))
                .and_then(|tc| tc.as_array())
            {
                for tc in tool_calls {
                    if let Some(func) = tc.get("function") {
                        let name = func
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let arguments = func
                            .get("arguments")
                            .and_then(|a| a.as_str())
                            .map(|s| s.to_string());
                        let call_id = tc
                            .get("id")
                            .and_then(|id| id.as_str())
                            .map(|s| s.to_string());
                        results.push(ToolCallInfo {
                            name,
                            arguments,
                            call_id,
                        });
                    }
                }
            }
        }
    }
    results
}

/// Anthropic: content[*] where type == "tool_use" → .name, .input
fn extract_anthropic_tool_calls(json: &Value) -> Vec<ToolCallInfo> {
    let mut results = vec![];
    if let Some(content) = json.get("content").and_then(|c| c.as_array()) {
        for item in content {
            if item.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                let name = item
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let arguments = item
                    .get("input")
                    .map(|v| serde_json::to_string(v).unwrap_or_default());
                let call_id = item
                    .get("id")
                    .and_then(|id| id.as_str())
                    .map(|s| s.to_string());
                results.push(ToolCallInfo {
                    name,
                    arguments,
                    call_id,
                });
            }
        }
    }
    results
}

/// Gemini: candidates[*].content.parts[*].functionCall.{name, args}
fn extract_gemini_tool_calls(json: &Value) -> Vec<ToolCallInfo> {
    let mut results = vec![];
    if let Some(candidates) = json.get("candidates").and_then(|c| c.as_array()) {
        for candidate in candidates {
            if let Some(parts) = candidate
                .get("content")
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.as_array())
            {
                for part in parts {
                    if let Some(fc) = part.get("functionCall") {
                        let name = fc
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let arguments = fc
                            .get("args")
                            .map(|v| serde_json::to_string(v).unwrap_or_default());
                        results.push(ToolCallInfo {
                            name,
                            arguments,
                            call_id: None,
                        });
                    }
                }
            }
        }
    }
    results
}

// ── Finish Reason ───────────────────────────────────────────────

/// Extract finish_reason from an LLM response.
#[allow(dead_code)]
pub fn extract_finish_reason(body: &[u8]) -> Option<String> {
    let json: Value = serde_json::from_slice(body).ok()?;
    extract_finish_reason_from_value(&json)
}

/// Extract finish_reason from an already-parsed JSON value.
pub fn extract_finish_reason_from_value(json: &Value) -> Option<String> {
    // OpenAI: choices[0].finish_reason
    if let Some(fr) = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("finish_reason"))
        .and_then(|f| f.as_str())
    {
        return Some(fr.to_string());
    }

    // Anthropic: stop_reason
    if let Some(sr) = json.get("stop_reason").and_then(|s| s.as_str()) {
        return Some(sr.to_string());
    }

    // Gemini: candidates[0].finishReason
    if let Some(fr) = json
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("finishReason"))
        .and_then(|f| f.as_str())
    {
        return Some(fr.to_lowercase());
    }

    None
}

// ── Error Classification ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LlmErrorType {
    RateLimit,
    ContextTooLong,
    InvalidAuth,
    ContentFilter,
    ServerError,
    Timeout,
    ModelNotFound,
    QuotaExceeded,
    InvalidRequest,
    Other,
}

impl std::fmt::Display for LlmErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RateLimit => write!(f, "rate_limit"),
            Self::ContextTooLong => write!(f, "context_too_long"),
            Self::InvalidAuth => write!(f, "invalid_auth"),
            Self::ContentFilter => write!(f, "content_filter"),
            Self::ServerError => write!(f, "server_error"),
            Self::Timeout => write!(f, "timeout"),
            Self::ModelNotFound => write!(f, "model_not_found"),
            Self::QuotaExceeded => write!(f, "quota_exceeded"),
            Self::InvalidRequest => write!(f, "invalid_request"),
            Self::Other => write!(f, "other"),
        }
    }
}

/// Classify an LLM error from status code and response body.
#[allow(dead_code)]
pub fn classify_error(status: u16, body: &[u8]) -> Option<String> {
    let body_str = std::str::from_utf8(body).unwrap_or("");
    let json: Option<Value> = serde_json::from_slice(body).ok();
    classify_error_inner(status, body_str, json.as_ref())
}

/// Classify an LLM error from status code and response body string.
pub fn classify_error_from_str(status: u16, body_str: &str) -> Option<String> {
    let json: Option<Value> = serde_json::from_str(body_str).ok();
    classify_error_inner(status, body_str, json.as_ref())
}

fn classify_error_inner(status: u16, body_str: &str, json: Option<&Value>) -> Option<String> {
    if status < 400 {
        return None; // Not an error
    }

    let error_message = json
        .and_then(|j| j.get("error"))
        .and_then(|e| e.get("message").or(e.get("type")))
        .and_then(|m| m.as_str())
        .unwrap_or("");
    let error_type_field = json
        .and_then(|j| j.get("error"))
        .and_then(|e| e.get("type"))
        .and_then(|t| t.as_str())
        .unwrap_or("");
    let error_code = json
        .and_then(|j| j.get("error"))
        .and_then(|e| e.get("code"))
        .and_then(|c| c.as_str())
        .unwrap_or("");

    let result = match status {
        401 => LlmErrorType::InvalidAuth,
        403 => LlmErrorType::InvalidAuth,
        404 => {
            if error_code.contains("model_not_found")
                || error_message.contains("does not exist")
                || error_message.contains("model")
            {
                LlmErrorType::ModelNotFound
            } else {
                LlmErrorType::Other
            }
        }
        429 => {
            if error_message.contains("quota")
                || error_code.contains("insufficient_quota")
                || error_type_field == "insufficient_quota"
            {
                LlmErrorType::QuotaExceeded
            } else {
                LlmErrorType::RateLimit
            }
        }
        400 => {
            let combined = format!(
                "{} {} {} {}",
                error_message, error_type_field, error_code, body_str
            );
            let lower = combined.to_lowercase();

            if lower.contains("context_length")
                || lower.contains("max_tokens")
                || lower.contains("maximum context")
                || lower.contains("too many tokens")
                || lower.contains("token limit")
            {
                LlmErrorType::ContextTooLong
            } else if lower.contains("content_filter")
                || lower.contains("content_policy")
                || lower.contains("safety")
                || lower.contains("flagged")
            {
                LlmErrorType::ContentFilter
            } else {
                LlmErrorType::InvalidRequest
            }
        }
        500..=599 => LlmErrorType::ServerError,
        _ => LlmErrorType::Other,
    };
    Some(result.to_string())
}

// ── Streaming Helpers ───────────────────────────────────────────

/// Check if the request body has `stream: true`.
pub fn is_streaming_request(body: &[u8]) -> bool {
    if let Ok(json) = serde_json::from_slice::<Value>(body) {
        json.get("stream")
            .and_then(|s| s.as_bool())
            .unwrap_or(false)
    } else {
        false
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Tool Call Extraction (bytes API) ────────────────────────

    #[test]
    fn test_openai_tool_calls() {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call_abc123",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"city\": \"London\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });
        let calls = extract_tool_calls(serde_json::to_string(&body).unwrap().as_bytes());
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].call_id.as_deref(), Some("call_abc123"));
        assert_eq!(
            calls[0].arguments.as_deref(),
            Some("{\"city\": \"London\"}")
        );
    }

    #[test]
    fn test_openai_multiple_tool_calls() {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "tool_calls": [
                        {
                            "id": "call_1",
                            "type": "function",
                            "function": { "name": "get_weather", "arguments": "{\"city\":\"NYC\"}" }
                        },
                        {
                            "id": "call_2",
                            "type": "function",
                            "function": { "name": "get_time", "arguments": "{\"tz\":\"EST\"}" }
                        },
                        {
                            "id": "call_3",
                            "type": "function",
                            "function": { "name": "search", "arguments": "{\"q\":\"hello\"}" }
                        }
                    ]
                },
                "finish_reason": "tool_calls"
            }]
        });
        let calls = extract_tool_calls(serde_json::to_string(&body).unwrap().as_bytes());
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[1].name, "get_time");
        assert_eq!(calls[2].name, "search");
        assert_eq!(calls[0].call_id.as_deref(), Some("call_1"));
        assert_eq!(calls[2].call_id.as_deref(), Some("call_3"));
    }

    #[test]
    fn test_anthropic_tool_calls() {
        let body = serde_json::json!({
            "content": [
                {"type": "text", "text": "I'll search for that."},
                {
                    "type": "tool_use",
                    "id": "toolu_01A",
                    "name": "search_database",
                    "input": {"query": "customer orders"}
                }
            ],
            "stop_reason": "tool_use"
        });
        let calls = extract_tool_calls(serde_json::to_string(&body).unwrap().as_bytes());
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "search_database");
        assert_eq!(calls[0].call_id.as_deref(), Some("toolu_01A"));
        assert!(calls[0]
            .arguments
            .as_ref()
            .unwrap()
            .contains("customer orders"));
    }

    #[test]
    fn test_anthropic_multiple_tool_calls() {
        let body = serde_json::json!({
            "content": [
                {"type": "text", "text": "Let me look that up."},
                {
                    "type": "tool_use",
                    "id": "toolu_01",
                    "name": "search_web",
                    "input": {"query": "rust programming"}
                },
                {
                    "type": "tool_use",
                    "id": "toolu_02",
                    "name": "read_file",
                    "input": {"path": "/tmp/data.txt"}
                }
            ],
            "stop_reason": "tool_use"
        });
        let calls = extract_tool_calls(serde_json::to_string(&body).unwrap().as_bytes());
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "search_web");
        assert_eq!(calls[1].name, "read_file");
    }

    #[test]
    fn test_gemini_tool_calls() {
        let body = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "functionCall": {
                            "name": "lookup_order",
                            "args": {"order_id": "12345"}
                        }
                    }]
                },
                "finishReason": "STOP"
            }]
        });
        let calls = extract_tool_calls(serde_json::to_string(&body).unwrap().as_bytes());
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "lookup_order");
        assert!(calls[0].arguments.as_ref().unwrap().contains("12345"));
        assert!(calls[0].call_id.is_none()); // Gemini doesn't have call IDs
    }

    #[test]
    fn test_gemini_multiple_function_calls() {
        let body = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [
                        { "functionCall": { "name": "get_weather", "args": {"city": "London"} } },
                        { "functionCall": { "name": "get_news", "args": {"topic": "tech"} } }
                    ]
                },
                "finishReason": "STOP"
            }]
        });
        let calls = extract_tool_calls(serde_json::to_string(&body).unwrap().as_bytes());
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[1].name, "get_news");
    }

    // ── Tool Call Extraction (_from_value API) ──────────────────

    #[test]
    fn test_tool_calls_from_value_openai() {
        let json = serde_json::json!({
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "call_v",
                        "function": { "name": "calc", "arguments": "{\"x\":1}" }
                    }]
                }
            }]
        });
        let calls = extract_tool_calls_from_value(&json);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "calc");
        assert_eq!(calls[0].call_id.as_deref(), Some("call_v"));
    }

    #[test]
    fn test_tool_calls_from_value_anthropic() {
        let json = serde_json::json!({
            "content": [{
                "type": "tool_use",
                "id": "toolu_v",
                "name": "deploy",
                "input": {"env": "prod"}
            }]
        });
        let calls = extract_tool_calls_from_value(&json);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "deploy");
    }

    // ── Edge Cases: No Tool Calls ───────────────────────────────

    #[test]
    fn test_no_tool_calls_text_response() {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Hello, how can I help?"
                },
                "finish_reason": "stop"
            }]
        });
        let calls = extract_tool_calls(serde_json::to_string(&body).unwrap().as_bytes());
        assert!(calls.is_empty());
    }

    #[test]
    fn test_no_tool_calls_invalid_json() {
        let calls = extract_tool_calls(b"this is not json");
        assert!(calls.is_empty());
    }

    #[test]
    fn test_no_tool_calls_empty_body() {
        let calls = extract_tool_calls(b"");
        assert!(calls.is_empty());
    }

    // ── Finish Reason Extraction ────────────────────────────────

    #[test]
    fn test_finish_reason_openai_stop() {
        let body = r#"{"choices":[{"finish_reason":"stop"}]}"#;
        assert_eq!(extract_finish_reason(body.as_bytes()), Some("stop".into()));
    }

    #[test]
    fn test_finish_reason_openai_tool_calls() {
        let body = r#"{"choices":[{"finish_reason":"tool_calls"}]}"#;
        assert_eq!(
            extract_finish_reason(body.as_bytes()),
            Some("tool_calls".into())
        );
    }

    #[test]
    fn test_finish_reason_openai_length() {
        let body = r#"{"choices":[{"finish_reason":"length"}]}"#;
        assert_eq!(
            extract_finish_reason(body.as_bytes()),
            Some("length".into())
        );
    }

    #[test]
    fn test_finish_reason_anthropic() {
        let body = r#"{"stop_reason":"end_turn"}"#;
        assert_eq!(
            extract_finish_reason(body.as_bytes()),
            Some("end_turn".into())
        );
    }

    #[test]
    fn test_finish_reason_anthropic_tool_use() {
        let body = r#"{"stop_reason":"tool_use"}"#;
        assert_eq!(
            extract_finish_reason(body.as_bytes()),
            Some("tool_use".into())
        );
    }

    #[test]
    fn test_finish_reason_gemini() {
        let body = r#"{"candidates":[{"finishReason":"STOP"}]}"#;
        assert_eq!(extract_finish_reason(body.as_bytes()), Some("stop".into()));
        // lowercased
    }

    #[test]
    fn test_finish_reason_gemini_safety() {
        let body = r#"{"candidates":[{"finishReason":"SAFETY"}]}"#;
        assert_eq!(
            extract_finish_reason(body.as_bytes()),
            Some("safety".into())
        );
    }

    #[test]
    fn test_finish_reason_from_value() {
        let json = serde_json::json!({"choices":[{"finish_reason":"stop"}]});
        assert_eq!(extract_finish_reason_from_value(&json), Some("stop".into()));

        let json2 = serde_json::json!({"stop_reason":"max_tokens"});
        assert_eq!(
            extract_finish_reason_from_value(&json2),
            Some("max_tokens".into())
        );
    }

    #[test]
    fn test_finish_reason_none_for_missing() {
        let body = r#"{"choices":[{"message":{"content":"hi"}}]}"#;
        assert_eq!(extract_finish_reason(body.as_bytes()), None);
    }

    #[test]
    fn test_finish_reason_none_for_invalid_json() {
        assert_eq!(extract_finish_reason(b"not json"), None);
    }

    // ── Error Classification ────────────────────────────────────

    #[test]
    fn test_error_not_an_error() {
        assert_eq!(classify_error(200, b"{}"), None);
        assert_eq!(classify_error(201, b"{}"), None);
        assert_eq!(classify_error(204, b"{}"), None);
    }

    #[test]
    fn test_error_invalid_auth_401() {
        let body = r#"{"error":{"message":"Invalid API key","type":"authentication_error"}}"#;
        assert_eq!(
            classify_error(401, body.as_bytes()),
            Some("invalid_auth".into())
        );
    }

    #[test]
    fn test_error_invalid_auth_403() {
        let body = r#"{"error":{"message":"Permission denied"}}"#;
        assert_eq!(
            classify_error(403, body.as_bytes()),
            Some("invalid_auth".into())
        );
    }

    #[test]
    fn test_error_rate_limit() {
        let body = r#"{"error":{"message":"Rate limit exceeded","type":"rate_limit_error"}}"#;
        assert_eq!(
            classify_error(429, body.as_bytes()),
            Some("rate_limit".into())
        );
    }

    #[test]
    fn test_error_quota_exceeded() {
        let body = r#"{"error":{"message":"You exceeded your current quota","type":"insufficient_quota"}}"#;
        assert_eq!(
            classify_error(429, body.as_bytes()),
            Some("quota_exceeded".into())
        );
    }

    #[test]
    fn test_error_quota_by_code() {
        let body = r#"{"error":{"message":"Billing limit reached","code":"insufficient_quota"}}"#;
        assert_eq!(
            classify_error(429, body.as_bytes()),
            Some("quota_exceeded".into())
        );
    }

    #[test]
    fn test_error_model_not_found() {
        let body =
            r#"{"error":{"message":"The model gpt-5 does not exist","code":"model_not_found"}}"#;
        assert_eq!(
            classify_error(404, body.as_bytes()),
            Some("model_not_found".into())
        );
    }

    #[test]
    fn test_error_404_generic() {
        let body = r#"{"error":{"message":"Not found"}}"#;
        assert_eq!(classify_error(404, body.as_bytes()), Some("other".into()));
    }

    #[test]
    fn test_error_context_too_long() {
        let body = r#"{"error":{"message":"This model's maximum context length is 128000 tokens","type":"invalid_request_error","code":"context_length_exceeded"}}"#;
        assert_eq!(
            classify_error(400, body.as_bytes()),
            Some("context_too_long".into())
        );
    }

    #[test]
    fn test_error_context_too_long_max_tokens() {
        let body = r#"{"error":{"message":"max_tokens exceeded for this model","type":"invalid_request_error"}}"#;
        assert_eq!(
            classify_error(400, body.as_bytes()),
            Some("context_too_long".into())
        );
    }

    #[test]
    fn test_error_content_filter() {
        let body =
            r#"{"error":{"message":"Content blocked by safety filter","code":"content_filter"}}"#;
        assert_eq!(
            classify_error(400, body.as_bytes()),
            Some("content_filter".into())
        );
    }

    #[test]
    fn test_error_content_policy() {
        let body = r#"{"error":{"message":"Your request was rejected as a result of our content_policy","type":"invalid_request_error"}}"#;
        assert_eq!(
            classify_error(400, body.as_bytes()),
            Some("content_filter".into())
        );
    }

    #[test]
    fn test_error_invalid_request_generic() {
        let body = r#"{"error":{"message":"Invalid parameter: temperature must be between 0 and 2","type":"invalid_request_error"}}"#;
        assert_eq!(
            classify_error(400, body.as_bytes()),
            Some("invalid_request".into())
        );
    }

    #[test]
    fn test_error_server_error_500() {
        let body = r#"{"error":{"message":"Internal server error"}}"#;
        assert_eq!(
            classify_error(500, body.as_bytes()),
            Some("server_error".into())
        );
    }

    #[test]
    fn test_error_server_error_503() {
        let body = r#"{"error":{"message":"Service temporarily unavailable"}}"#;
        assert_eq!(
            classify_error(503, body.as_bytes()),
            Some("server_error".into())
        );
    }

    #[test]
    fn test_error_other_status() {
        assert_eq!(classify_error(418, b"{}"), Some("other".into()));
    }

    // ── classify_error_from_str ─────────────────────────────────

    #[test]
    fn test_classify_error_from_str() {
        let body_str = r#"{"error":{"message":"Rate limit exceeded","type":"rate_limit_error"}}"#;
        assert_eq!(
            classify_error_from_str(429, body_str),
            Some("rate_limit".into())
        );
    }

    #[test]
    fn test_classify_error_from_str_not_error() {
        assert_eq!(classify_error_from_str(200, "{}"), None);
    }

    #[test]
    fn test_classify_error_from_str_invalid_json() {
        // Should still classify by status code even with invalid JSON body
        assert_eq!(
            classify_error_from_str(500, "not json"),
            Some("server_error".into())
        );
    }

    // ── Streaming Request Detection ─────────────────────────────

    #[test]
    fn test_streaming_request_true() {
        assert!(is_streaming_request(
            b"{\"stream\": true, \"model\": \"gpt-4o\"}"
        ));
    }

    #[test]
    fn test_streaming_request_false() {
        assert!(!is_streaming_request(b"{\"stream\": false}"));
    }

    #[test]
    fn test_streaming_request_missing() {
        assert!(!is_streaming_request(b"{\"model\": \"gpt-4o\"}"));
    }

    #[test]
    fn test_streaming_request_invalid_json() {
        assert!(!is_streaming_request(b"not json"));
    }

    #[test]
    fn test_streaming_request_empty() {
        assert!(!is_streaming_request(b""));
    }

    #[test]
    fn test_streaming_request_string_value() {
        // stream: "true" (string) should return false—only boolean true counts
        assert!(!is_streaming_request(b"{\"stream\": \"true\"}"));
    }

    // ── ToolCallInfo Serialization ──────────────────────────────

    #[test]
    fn test_tool_call_info_serialization() {
        let tc = ToolCallInfo {
            name: "get_weather".into(),
            arguments: Some("{\"city\":\"NYC\"}".into()),
            call_id: Some("call_123".into()),
        };
        let json = serde_json::to_value(&tc).unwrap();
        assert_eq!(json["name"], "get_weather");
        assert_eq!(json["arguments"], "{\"city\":\"NYC\"}");
        assert_eq!(json["call_id"], "call_123");
    }

    #[test]
    fn test_tool_call_info_skip_none_fields() {
        let tc = ToolCallInfo {
            name: "search".into(),
            arguments: None,
            call_id: None,
        };
        let json = serde_json::to_value(&tc).unwrap();
        assert_eq!(json["name"], "search");
        assert!(json.get("arguments").is_none());
        assert!(json.get("call_id").is_none());
    }

    // ── LlmErrorType Display ────────────────────────────────────

    #[test]
    fn test_error_type_display() {
        assert_eq!(LlmErrorType::RateLimit.to_string(), "rate_limit");
        assert_eq!(LlmErrorType::QuotaExceeded.to_string(), "quota_exceeded");
        assert_eq!(LlmErrorType::InvalidAuth.to_string(), "invalid_auth");
        assert_eq!(LlmErrorType::ContextTooLong.to_string(), "context_too_long");
        assert_eq!(LlmErrorType::ContentFilter.to_string(), "content_filter");
        assert_eq!(LlmErrorType::ModelNotFound.to_string(), "model_not_found");
        assert_eq!(LlmErrorType::InvalidRequest.to_string(), "invalid_request");
        assert_eq!(LlmErrorType::ServerError.to_string(), "server_error");
        assert_eq!(LlmErrorType::Other.to_string(), "other");
    }
}
