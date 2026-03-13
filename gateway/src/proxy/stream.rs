//! SSE Stream Accumulator for LLM observability.
//!
//! Buffers Server-Sent Events chunks from streaming LLM responses,
//! passes them through to the client in real-time, and reassembles
//! the complete response for logging, tool call extraction, and cost tracking.

use crate::models::llm::ToolCallInfo;
use serde_json::Value;
use std::time::Instant;

/// Result of accumulating a complete SSE stream.
#[derive(Debug)]
pub struct StreamResult {
    /// Concatenated text content from all chunks
    pub content: String,
    /// Tool calls assembled from streaming deltas
    pub tool_calls: Vec<ToolCallInfo>,
    /// Usage extracted from the final chunk (if provider includes it)
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    /// Model name
    pub model: Option<String>,
    /// Finish reason from the final chunk
    pub finish_reason: Option<String>,
    /// Time to first token (TTFT) in milliseconds
    pub ttft_ms: Option<u64>,
    /// Total chunks received
    #[allow(dead_code)]
    pub chunk_count: u32,
}

/// Accumulates SSE chunks from a streaming LLM response.
pub struct StreamAccumulator {
    /// Text content being assembled
    content: String,
    /// Tool call deltas being assembled (OpenAI sends tool calls incrementally)
    tool_call_deltas: Vec<ToolCallDelta>,
    /// Usage from final chunk
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    /// Model (usually in first chunk)
    model: Option<String>,
    /// Finish reason from final chunk
    finish_reason: Option<String>,
    /// When the stream started
    start_time: Instant,
    /// When the first content chunk arrived
    first_chunk_at: Option<Instant>,
    /// Total chunks processed
    chunk_count: u32,
}

#[derive(Debug, Clone)]
struct ToolCallDelta {
    #[allow(dead_code)]
    index: usize,
    call_id: Option<String>,
    name: String,
    arguments: String,
}

impl StreamAccumulator {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            tool_call_deltas: Vec::new(),
            prompt_tokens: None,
            completion_tokens: None,
            model: None,
            finish_reason: None,
            start_time: Instant::now(),
            first_chunk_at: None,
            chunk_count: 0,
        }
    }

    /// Create a new accumulator with an externally-provided start time.
    /// Used by `stream_bridge` so TTFT is measured from the original request,
    /// not from when the accumulator was constructed.
    pub fn new_with_start(start: Instant) -> Self {
        Self {
            start_time: start,
            ..Self::new()
        }
    }

    /// Explicitly set the TTFT in milliseconds (called by stream_bridge
    /// when the first byte arrives from upstream).
    pub fn set_ttft_ms(&mut self, ms: u64) {
        // Only set if not already recorded via first_chunk_at
        if self.first_chunk_at.is_none() {
            // Store as a synthetic first_chunk_at offset from start_time
            self.first_chunk_at = Some(self.start_time + std::time::Duration::from_millis(ms));
        }
    }

    /// Alias for `finalize` — used by `stream_bridge`.
    pub fn finish(self) -> StreamResult {
        self.finalize()
    }

    /// Process a single SSE line like `data: {"choices":[...]}`
    /// Returns true if this is the terminal `[DONE]` marker.
    pub fn push_sse_line(&mut self, line: &str) -> bool {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with(':') {
            return false;
        }

        // Extract data payload
        let data = if let Some(stripped) = line.strip_prefix("data: ") {
            stripped.trim()
        } else if let Some(stripped) = line.strip_prefix("data:") {
            stripped.trim()
        } else {
            return false;
        };

        // Terminal marker
        if data == "[DONE]" {
            return true;
        }

        // Parse JSON chunk
        let json: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => return false,
        };

        // FIX M-3: Only count successfully parsed chunks
        self.chunk_count += 1;

        // Extract model (usually in first chunk)
        if self.model.is_none() {
            if let Some(m) = json.get("model").and_then(|v| v.as_str()) {
                self.model = Some(m.to_string());
            }
        }

        // Try OpenAI/compatible format
        self.process_openai_chunk(&json);

        // Try Anthropic streaming format
        self.process_anthropic_chunk(&json);

        // Check for usage in this chunk (OpenAI includes it in final chunk
        // when stream_options.include_usage is true)
        self.extract_chunk_usage(&json);

        false
    }

    /// Process an OpenAI-format streaming chunk.
    /// Format: {"choices":[{"delta":{"content":"...","tool_calls":[...]}, "finish_reason":"..."}]}
    fn process_openai_chunk(&mut self, json: &Value) {
        if let Some(choices) = json.get("choices").and_then(|c| c.as_array()) {
            for choice in choices {
                // Extract finish_reason
                if let Some(fr) = choice.get("finish_reason").and_then(|f| f.as_str()) {
                    self.finish_reason = Some(fr.to_string());
                }

                let delta = match choice.get("delta") {
                    Some(d) => d,
                    None => continue,
                };

                // Extract content delta
                if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                    if self.first_chunk_at.is_none() && !content.is_empty() {
                        self.first_chunk_at = Some(Instant::now());
                    }
                    self.content.push_str(content);
                }

                // Extract tool call deltas (OpenAI sends these incrementally)
                if let Some(tool_calls) = delta.get("tool_calls").and_then(|tc| tc.as_array()) {
                    for tc in tool_calls {
                        let index = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;

                        // Find or create the delta entry for this index
                        while self.tool_call_deltas.len() <= index {
                            self.tool_call_deltas.push(ToolCallDelta {
                                index: self.tool_call_deltas.len(),
                                call_id: None,
                                name: String::new(),
                                arguments: String::new(),
                            });
                        }

                        let entry = &mut self.tool_call_deltas[index];

                        if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                            entry.call_id = Some(id.to_string());
                        }

                        if let Some(func) = tc.get("function") {
                            if let Some(name) = func.get("name").and_then(|n| n.as_str()) {
                                entry.name.push_str(name);
                            }
                            if let Some(args) = func.get("arguments").and_then(|a| a.as_str()) {
                                entry.arguments.push_str(args);
                            }
                        }

                        // Record TTFT for tool calls too
                        if self.first_chunk_at.is_none() {
                            self.first_chunk_at = Some(Instant::now());
                        }
                    }
                }
            }
        }
    }

    /// Process an Anthropic-format streaming event.
    /// Events: content_block_start, content_block_delta, content_block_stop, message_delta, message_stop
    fn process_anthropic_chunk(&mut self, json: &Value) {
        let event_type = match json.get("type").and_then(|t| t.as_str()) {
            Some(t) => t,
            None => return,
        };

        match event_type {
            "content_block_start" => {
                // Could be text or tool_use
                if let Some(cb) = json.get("content_block") {
                    if cb.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                        let name = cb
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let call_id = cb
                            .get("id")
                            .and_then(|id| id.as_str())
                            .map(|s| s.to_string());
                        let index =
                            json.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                        while self.tool_call_deltas.len() <= index {
                            self.tool_call_deltas.push(ToolCallDelta {
                                index: self.tool_call_deltas.len(),
                                call_id: None,
                                name: String::new(),
                                arguments: String::new(),
                            });
                        }
                        self.tool_call_deltas[index] = ToolCallDelta {
                            index,
                            call_id,
                            name,
                            arguments: String::new(),
                        };
                    }
                }
            }
            "content_block_delta" => {
                if let Some(delta) = json.get("delta") {
                    // Text delta
                    if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                        if self.first_chunk_at.is_none() && !text.is_empty() {
                            self.first_chunk_at = Some(Instant::now());
                        }
                        self.content.push_str(text);
                    }
                    // Tool input delta
                    if let Some(partial) = delta.get("partial_json").and_then(|p| p.as_str()) {
                        let index =
                            json.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                        if index < self.tool_call_deltas.len() {
                            self.tool_call_deltas[index].arguments.push_str(partial);
                        }
                        if self.first_chunk_at.is_none() {
                            self.first_chunk_at = Some(Instant::now());
                        }
                    }
                }
            }
            "message_delta" => {
                if let Some(delta) = json.get("delta") {
                    if let Some(sr) = delta.get("stop_reason").and_then(|s| s.as_str()) {
                        self.finish_reason = Some(sr.to_string());
                    }
                }
                // Anthropic includes usage in message_delta
                if let Some(usage) = json.get("usage") {
                    if let Some(out) = usage.get("output_tokens").and_then(|v| v.as_u64()) {
                        self.completion_tokens = Some(out as u32);
                    }
                }
            }
            "message_start" => {
                // Anthropic: extract input tokens from message_start
                if let Some(message) = json.get("message") {
                    if let Some(model) = message.get("model").and_then(|m| m.as_str()) {
                        self.model = Some(model.to_string());
                    }
                    if let Some(usage) = message.get("usage") {
                        if let Some(inp) = usage.get("input_tokens").and_then(|v| v.as_u64()) {
                            self.prompt_tokens = Some(inp as u32);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Extract usage from a chunk (OpenAI final chunk with stream_options.include_usage).
    fn extract_chunk_usage(&mut self, json: &Value) {
        if let Some(usage) = json.get("usage") {
            if let Some(pt) = usage
                .get("prompt_tokens")
                .or_else(|| usage.get("input_tokens"))
                .and_then(|v| v.as_u64())
            {
                self.prompt_tokens = Some(pt as u32);
            }
            if let Some(ct) = usage
                .get("completion_tokens")
                .or_else(|| usage.get("output_tokens"))
                .and_then(|v| v.as_u64())
            {
                self.completion_tokens = Some(ct as u32);
            }
        }

        // Gemini: extract usageMetadata (present in every chunk, use last value)
        if let Some(usage_meta) = json.get("usageMetadata") {
            if let Some(pt) = usage_meta.get("promptTokenCount").and_then(|v| v.as_u64()) {
                self.prompt_tokens = Some(pt as u32);
            }
            if let Some(ct) = usage_meta
                .get("candidatesTokenCount")
                .and_then(|v| v.as_u64())
            {
                self.completion_tokens = Some(ct as u32);
            }
        }
    }

    /// Finalize the stream and produce a `StreamResult`.
    pub fn finalize(self) -> StreamResult {
        let tool_calls: Vec<ToolCallInfo> = self
            .tool_call_deltas
            .into_iter()
            .filter(|d| !d.name.is_empty())
            .map(|d| ToolCallInfo {
                name: d.name,
                arguments: if d.arguments.is_empty() {
                    None
                } else {
                    Some(d.arguments)
                },
                call_id: d.call_id,
            })
            .collect();

        let ttft_ms = self
            .first_chunk_at
            .map(|t| t.duration_since(self.start_time).as_millis() as u64);

        StreamResult {
            content: self.content,
            tool_calls,
            prompt_tokens: self.prompt_tokens,
            completion_tokens: self.completion_tokens,
            model: self.model,
            finish_reason: self.finish_reason,
            ttft_ms,
            chunk_count: self.chunk_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── OpenAI Text Streaming ───────────────────────────────────

    #[test]
    fn test_openai_streaming_text() {
        let mut acc = StreamAccumulator::new();

        // Simulate OpenAI streaming chunks
        assert!(!acc.push_sse_line(
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"},\"index\":0}]}"
        ));
        assert!(!acc.push_sse_line(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"},\"index\":0}]}"
        ));
        assert!(!acc.push_sse_line(
            "data: {\"choices\":[{\"delta\":{\"content\":\" world\"},\"index\":0}]}"
        ));
        assert!(!acc.push_sse_line(
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\",\"index\":0}]}"
        ));
        assert!(acc.push_sse_line("data: [DONE]"));

        let result = acc.finalize();
        assert_eq!(result.content, "Hello world");
        assert_eq!(result.finish_reason.as_deref(), Some("stop"));
        assert!(result.tool_calls.is_empty());
        assert_eq!(result.chunk_count, 4);
    }

    #[test]
    fn test_openai_streaming_long_text() {
        let mut acc = StreamAccumulator::new();
        // Simulate many small chunks (realistic for long responses)
        let words = [
            "The ", "quick ", "brown ", "fox ", "jumps ", "over ", "the ", "lazy ", "dog.",
        ];
        for w in &words {
            acc.push_sse_line(&format!(
                "data: {{\"choices\":[{{\"delta\":{{\"content\":\"{w}\"}},\"index\":0}}]}}"
            ));
        }
        acc.push_sse_line(
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\",\"index\":0}]}",
        );
        acc.push_sse_line("data: [DONE]");

        let result = acc.finalize();
        assert_eq!(
            result.content,
            "The quick brown fox jumps over the lazy dog."
        );
        assert_eq!(result.chunk_count, 10); // 9 content + 1 finish
        assert_eq!(result.finish_reason.as_deref(), Some("stop"));
    }

    // ── OpenAI Tool Call Streaming ──────────────────────────────

    #[test]
    fn test_openai_streaming_tool_calls() {
        let mut acc = StreamAccumulator::new();

        // OpenAI sends tool calls incrementally
        acc.push_sse_line("data: {\"model\":\"gpt-4o\",\"choices\":[{\"delta\":{\"role\":\"assistant\",\"tool_calls\":[{\"index\":0,\"id\":\"call_abc\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"\"}}]},\"index\":0}]}");
        acc.push_sse_line("data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"ci\"}}]},\"index\":0}]}");
        acc.push_sse_line("data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"ty\\\": \\\"\"}}]},\"index\":0}]}");
        acc.push_sse_line("data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"London\\\"}\"}}]},\"index\":0}]}");
        acc.push_sse_line(
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\",\"index\":0}]}",
        );
        acc.push_sse_line("data: [DONE]");

        let result = acc.finalize();
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name, "get_weather");
        assert_eq!(result.tool_calls[0].call_id.as_deref(), Some("call_abc"));
        assert_eq!(
            result.tool_calls[0].arguments.as_deref(),
            Some("{\"city\": \"London\"}")
        );
        assert_eq!(result.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(result.model.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn test_openai_streaming_multiple_tool_calls() {
        let mut acc = StreamAccumulator::new();

        // First tool call starts
        acc.push_sse_line("data: {\"model\":\"gpt-4o\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"search\",\"arguments\":\"\"}}]},\"index\":0}]}");
        acc.push_sse_line("data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"q\\\":\\\"rust\\\"}\"}}]},\"index\":0}]}");
        // Second tool call starts
        acc.push_sse_line("data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":1,\"id\":\"call_2\",\"function\":{\"name\":\"read_file\",\"arguments\":\"\"}}]},\"index\":0}]}");
        acc.push_sse_line("data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":1,\"function\":{\"arguments\":\"{\\\"path\\\":\\\"/tmp\\\"}\"}}]},\"index\":0}]}");
        acc.push_sse_line(
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\",\"index\":0}]}",
        );
        acc.push_sse_line("data: [DONE]");

        let result = acc.finalize();
        assert_eq!(result.tool_calls.len(), 2);
        assert_eq!(result.tool_calls[0].name, "search");
        assert_eq!(result.tool_calls[0].call_id.as_deref(), Some("call_1"));
        assert_eq!(
            result.tool_calls[0].arguments.as_deref(),
            Some("{\"q\":\"rust\"}")
        );
        assert_eq!(result.tool_calls[1].name, "read_file");
        assert_eq!(result.tool_calls[1].call_id.as_deref(), Some("call_2"));
        assert_eq!(
            result.tool_calls[1].arguments.as_deref(),
            Some("{\"path\":\"/tmp\"}")
        );
    }

    // ── OpenAI Usage Extraction ─────────────────────────────────

    #[test]
    fn test_openai_usage_in_final_chunk() {
        let mut acc = StreamAccumulator::new();
        acc.push_sse_line("data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}");
        acc.push_sse_line("data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5}}");
        acc.push_sse_line("data: [DONE]");

        let result = acc.finalize();
        assert_eq!(result.prompt_tokens, Some(10));
        assert_eq!(result.completion_tokens, Some(5));
    }

    #[test]
    fn test_openai_no_usage_without_stream_options() {
        let mut acc = StreamAccumulator::new();
        acc.push_sse_line("data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}");
        acc.push_sse_line("data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}");
        acc.push_sse_line("data: [DONE]");

        let result = acc.finalize();
        assert_eq!(result.prompt_tokens, None);
        assert_eq!(result.completion_tokens, None);
    }

    // ── Anthropic Streaming ─────────────────────────────────────

    #[test]
    fn test_anthropic_streaming() {
        let mut acc = StreamAccumulator::new();

        acc.push_sse_line("data: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-3-5-sonnet-20241022\",\"usage\":{\"input_tokens\":25}}}");
        acc.push_sse_line("data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}");
        acc.push_sse_line("data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}");
        acc.push_sse_line("data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" there\"}}");
        acc.push_sse_line("data: {\"type\":\"content_block_stop\",\"index\":0}");
        acc.push_sse_line("data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":12}}");
        acc.push_sse_line("data: {\"type\":\"message_stop\"}");

        let result = acc.finalize();
        assert_eq!(result.content, "Hello there");
        assert_eq!(result.model.as_deref(), Some("claude-3-5-sonnet-20241022"));
        assert_eq!(result.prompt_tokens, Some(25));
        assert_eq!(result.completion_tokens, Some(12));
        assert_eq!(result.finish_reason.as_deref(), Some("end_turn"));
    }

    #[test]
    fn test_anthropic_streaming_tool_use() {
        let mut acc = StreamAccumulator::new();

        acc.push_sse_line("data: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-3-5-sonnet-20241022\",\"usage\":{\"input_tokens\":30}}}");
        acc.push_sse_line("data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_01\",\"name\":\"search\",\"input\":{}}}");
        acc.push_sse_line("data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"query\\\":\"}}");
        acc.push_sse_line("data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\" \\\"hello\\\"}\"}}");
        acc.push_sse_line("data: {\"type\":\"content_block_stop\",\"index\":0}");
        acc.push_sse_line("data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":8}}");

        let result = acc.finalize();
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name, "search");
        assert_eq!(result.tool_calls[0].call_id.as_deref(), Some("toolu_01"));
        assert_eq!(result.finish_reason.as_deref(), Some("tool_use"));
    }

    #[test]
    fn test_anthropic_mixed_text_and_tool() {
        let mut acc = StreamAccumulator::new();

        acc.push_sse_line("data: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-3-5-sonnet-20241022\",\"usage\":{\"input_tokens\":50}}}");
        // First block: text
        acc.push_sse_line("data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}");
        acc.push_sse_line("data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"I'll look that up.\"}}");
        acc.push_sse_line("data: {\"type\":\"content_block_stop\",\"index\":0}");
        // Second block: tool_use
        acc.push_sse_line("data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_02\",\"name\":\"web_search\",\"input\":{}}}");
        acc.push_sse_line("data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"q\\\":\\\"rust lang\\\"}\"}}");
        acc.push_sse_line("data: {\"type\":\"content_block_stop\",\"index\":1}");
        acc.push_sse_line("data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":20}}");

        let result = acc.finalize();
        assert_eq!(result.content, "I'll look that up.");
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name, "web_search");
        assert_eq!(result.tool_calls[0].call_id.as_deref(), Some("toolu_02"));
        assert_eq!(result.prompt_tokens, Some(50));
        assert_eq!(result.completion_tokens, Some(20));
    }

    // ── Edge Cases ──────────────────────────────────────────────

    #[test]
    fn test_done_marker() {
        let mut acc = StreamAccumulator::new();
        assert!(!acc.push_sse_line("data: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}"));
        assert!(acc.push_sse_line("data: [DONE]"));
    }

    #[test]
    fn test_ignores_comments_and_empty() {
        let mut acc = StreamAccumulator::new();
        assert!(!acc.push_sse_line(""));
        assert!(!acc.push_sse_line(": keep-alive"));
        assert!(!acc.push_sse_line("event: message"));
        assert_eq!(acc.chunk_count, 0);
    }

    #[test]
    fn test_empty_stream_only_done() {
        let mut acc = StreamAccumulator::new();
        assert!(acc.push_sse_line("data: [DONE]"));

        let result = acc.finalize();
        assert_eq!(result.content, "");
        assert!(result.tool_calls.is_empty());
        assert_eq!(result.chunk_count, 0);
        assert_eq!(result.finish_reason, None);
        assert_eq!(result.model, None);
    }

    #[test]
    fn test_malformed_json_chunks_skipped() {
        let mut acc = StreamAccumulator::new();
        // Malformed JSON should be skipped without panic
        assert!(!acc.push_sse_line("data: {not valid json}"));
        assert!(!acc.push_sse_line("data: "));
        // Valid chunk after malformed ones should still work
        assert!(!acc.push_sse_line("data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}"));
        assert!(acc.push_sse_line("data: [DONE]"));

        let result = acc.finalize();
        assert_eq!(result.content, "ok");
        // chunk_count increments before JSON parse, so malformed chunks are counted
        // but their data is safely discarded
        assert!(result.chunk_count >= 1);
    }

    #[test]
    fn test_data_prefix_without_space() {
        // Some SSE implementations use "data:" (no space)
        let mut acc = StreamAccumulator::new();
        assert!(!acc.push_sse_line("data:{\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}"));
        let result = acc.finalize();
        assert_eq!(result.content, "hello");
    }

    #[test]
    fn test_model_extraction_from_first_chunk() {
        let mut acc = StreamAccumulator::new();
        acc.push_sse_line(
            "data: {\"model\":\"gpt-4o-mini\",\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}",
        );
        acc.push_sse_line("data: {\"choices\":[{\"delta\":{\"content\":\" there\"}}]}");
        acc.push_sse_line("data: [DONE]");

        let result = acc.finalize();
        assert_eq!(result.model.as_deref(), Some("gpt-4o-mini"));
    }

    #[test]
    fn test_ttft_is_set() {
        let mut acc = StreamAccumulator::new();
        // First chunk has content → TTFT should be set
        acc.push_sse_line("data: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}");
        acc.push_sse_line("data: [DONE]");

        let result = acc.finalize();
        // TTFT should be Some (we can't check exact value but it should be set)
        assert!(result.ttft_ms.is_some());
    }

    #[test]
    fn test_ttft_not_set_for_empty_content() {
        let mut acc = StreamAccumulator::new();
        // Empty content string
        acc.push_sse_line("data: {\"choices\":[{\"delta\":{\"content\":\"\"}}]}");
        // Role-only delta (no content)
        acc.push_sse_line("data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}");
        acc.push_sse_line("data: [DONE]");

        let result = acc.finalize();
        // No actual content was delivered, TTFT should be None
        assert!(result.ttft_ms.is_none());
    }

    #[test]
    fn test_finish_reason_length() {
        let mut acc = StreamAccumulator::new();
        acc.push_sse_line("data: {\"choices\":[{\"delta\":{\"content\":\"truncated\"}}]}");
        acc.push_sse_line("data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}");
        acc.push_sse_line("data: [DONE]");

        let result = acc.finalize();
        assert_eq!(result.finish_reason.as_deref(), Some("length"));
    }

    #[test]
    fn test_whitespace_handling() {
        let mut acc = StreamAccumulator::new();
        // Lines with leading/trailing whitespace
        assert!(!acc.push_sse_line("  data: {\"choices\":[{\"delta\":{\"content\":\"ws\"}}]}  "));
        let result = acc.finalize();
        assert_eq!(result.content, "ws");
    }

    // ── Chaos: Stream Abort & Resilience ────────────────────────

    /// Client disconnects after 3 chunks — no [DONE] marker.
    /// Accumulator must finalize cleanly with partial content.
    #[test]
    fn test_stream_abort_preserves_partial_content() {
        let mut acc = StreamAccumulator::new_with_start(std::time::Instant::now());
        let chunks = [
            r#"data: {"choices":[{"delta":{"content":"Hello"},"index":0}]}"#,
            r#"data: {"choices":[{"delta":{"content":" cruel"},"index":0}]}"#,
            r#"data: {"choices":[{"delta":{"content":" world"},"index":0}]}"#,
        ];
        for chunk in &chunks {
            assert!(
                !acc.push_sse_line(chunk),
                "should not be done on partial stream"
            );
        }
        let result = acc.finalize();
        assert_eq!(result.content, "Hello cruel world");
        assert_eq!(result.chunk_count, 3);
        assert!(result.finish_reason.is_none(), "no finish_reason on abort");
    }

    /// Immediate abort — zero chunks received, finalize shouldn't panic.
    #[test]
    fn test_stream_empty_finalize_no_panic() {
        let acc = StreamAccumulator::new_with_start(std::time::Instant::now());
        let result = acc.finalize();
        assert_eq!(result.content, "");
        assert_eq!(result.chunk_count, 0);
        assert!(result.tool_calls.is_empty());
    }

    /// Stream with only SSE comments / keep-alives — no data payload.
    #[test]
    fn test_stream_only_comments_no_data() {
        let mut acc = StreamAccumulator::new_with_start(std::time::Instant::now());
        acc.push_sse_line(": keep-alive");
        acc.push_sse_line("");
        acc.push_sse_line(": another comment");
        let result = acc.finalize();
        assert_eq!(result.content, "");
        assert_eq!(result.chunk_count, 0);
    }

    /// Usage in the final chunk must be captured (OpenAI stream_options.include_usage).
    #[test]
    fn test_stream_usage_captured_from_final_chunk() {
        let mut acc = StreamAccumulator::new_with_start(std::time::Instant::now());
        acc.push_sse_line(r#"data: {"choices":[{"delta":{"content":"Hi"}}]}"#);
        acc.push_sse_line(r#"data: {"choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#);
        acc.push_sse_line("data: [DONE]");
        let result = acc.finalize();
        assert_eq!(result.content, "Hi");
        assert_eq!(result.prompt_tokens, Some(10));
        assert_eq!(result.completion_tokens, Some(5));
    }
}
