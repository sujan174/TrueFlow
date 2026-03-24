//! Prompt Guardrails — content safety middleware.
//!
//! Implements `Action::ContentFilter` for the condition→action engine.
//! Detects jailbreak attempts, harmful content, off-topic prompts,
//! profanity, bias, competitor mentions, sensitive topics, gibberish,
//! contact info exposure, and IP leakage.
//!
//! # Design
//! - **10 built-in categories** with 100+ compiled regex patterns.
//! - **Topic filtering**: keyword-based allow/deny lists against message content.
//! - **Custom patterns**: policy authors can supply additional regex strings.
//! - **Risk scoring**: 0.0–1.0 composite score; threshold configurable per policy.

mod patterns;
mod schema;

#[cfg(test)]
mod tests;

use serde_json::Value;

use self::patterns::*;
use crate::models::policy::Action;

pub use self::schema::validate_schema;

// ── Public Types ──────────────────────────────────────────────

/// Result of a guardrail content check.
#[derive(Debug, Clone)]
pub struct GuardrailResult {
    /// Whether the request should be blocked.
    pub blocked: bool,
    /// Human-readable reason for blocking (for audit log + error response).
    pub reason: Option<String>,
    /// Names of the patterns that matched (for audit log).
    pub matched_patterns: Vec<String>,
    /// Composite risk score 0.0–1.0.
    pub risk_score: f32,
}

impl GuardrailResult {
    fn allow() -> Self {
        Self {
            blocked: false,
            reason: None,
            matched_patterns: vec![],
            risk_score: 0.0,
        }
    }

    fn block(reason: impl Into<String>, patterns: Vec<String>, score: f32) -> Self {
        Self {
            blocked: true,
            reason: Some(reason.into()),
            matched_patterns: patterns,
            risk_score: score,
        }
    }
}

// ── Regex Cache ─────────────────────────────────────────────────

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    /// Thread-local cache: pattern string → compiled Regex (None = invalid/too-complex).
    /// Bounded at 256 entries to prevent unbounded memory growth from malicious policies.
    /// Shared by topic denylist, allowlist, and custom patterns for performance.
    static GUARDRAIL_REGEX_CACHE: RefCell<HashMap<String, Option<regex::Regex>>> =
        RefCell::new(HashMap::with_capacity(64));
}

/// Compile a regex pattern with size limit, caching the result per-thread.
/// Returns None if the pattern is invalid or too complex (ReDoS protection).
fn compile_cached_guardrail(pat: &str) -> Option<regex::Regex> {
    GUARDRAIL_REGEX_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(cached) = cache.get(pat) {
            return cached.clone();
        }
        let compiled = regex::RegexBuilder::new(pat)
            .size_limit(1_000_000)
            .build()
            .ok();
        if cache.len() >= 256 {
            cache.clear();
        }
        cache.insert(pat.to_string(), compiled.clone());
        compiled
    })
}

// ── Main Entry Point ──────────────────────────────────────────

/// Check a request body against the `ContentFilter` action config.
///
/// Extracts all message content from the body (OpenAI format) and runs
/// the configured checks in order:
/// harmful → code → jailbreak → profanity → bias → sensitive_topics →
/// gibberish → contact_info → ip_leakage → competitor → topic → custom → length.
pub fn check_content(body: &Value, action: &Action) -> GuardrailResult {
    let (
        block_jailbreak,
        block_harmful,
        block_code_injection,
        block_profanity,
        block_bias,
        block_competitor_mention,
        block_sensitive_topics,
        block_gibberish,
        block_contact_info,
        block_ip_leakage,
        competitor_names,
        topic_allowlist,
        topic_denylist,
        custom_patterns,
        risk_threshold,
        max_content_length,
    ) = match action {
        Action::ContentFilter {
            block_jailbreak,
            block_harmful,
            block_code_injection,
            block_profanity,
            block_bias,
            block_competitor_mention,
            block_sensitive_topics,
            block_gibberish,
            block_contact_info,
            block_ip_leakage,
            competitor_names,
            topic_allowlist,
            topic_denylist,
            custom_patterns,
            risk_threshold,
            max_content_length,
        } => (
            *block_jailbreak,
            *block_harmful,
            *block_code_injection,
            *block_profanity,
            *block_bias,
            *block_competitor_mention,
            *block_sensitive_topics,
            *block_gibberish,
            *block_contact_info,
            *block_ip_leakage,
            competitor_names,
            topic_allowlist,
            topic_denylist,
            custom_patterns,
            *risk_threshold,
            *max_content_length,
        ),
        _ => return GuardrailResult::allow(),
    };

    // Extract all text content from the request body
    let text = extract_text_content(body);
    if text.is_empty() {
        return GuardrailResult::allow();
    }

    // 1. Harmful content check (highest priority — always block regardless of threshold)
    if block_harmful {
        let matches: Vec<usize> = HARMFUL_SET.matches(&text).into_iter().collect();
        if !matches.is_empty() {
            return GuardrailResult::block(
                "Request blocked: harmful content detected",
                matches
                    .iter()
                    .map(|i| format!("harmful_pattern_{}", i))
                    .collect(),
                1.0,
            );
        }
    }

    // 2. Code injection detection
    let mut matched_patterns: Vec<String> = vec![];
    let mut risk_score: f32 = 0.0;

    if block_code_injection {
        let code_matches: Vec<usize> = CODE_INJECTION_SET.matches(&text).into_iter().collect();
        if !code_matches.is_empty() {
            let pattern_names: Vec<String> = code_matches
                .iter()
                .map(|i| format!("code_injection_{}", i))
                .collect();
            risk_score = (risk_score + code_matches.len() as f32 * 0.5).min(1.0);
            matched_patterns.extend(pattern_names);
        }
    }

    // 3. Jailbreak detection
    if block_jailbreak {
        let jailbreak_matches: Vec<usize> = JAILBREAK_SET.matches(&text).into_iter().collect();
        if !jailbreak_matches.is_empty() {
            let pattern_names: Vec<String> = jailbreak_matches
                .iter()
                .map(|i| format!("jailbreak_{}", i))
                .collect();
            risk_score = (risk_score + jailbreak_matches.len() as f32 * 0.5).min(1.0);
            matched_patterns.extend(pattern_names);
        }
    }

    // 4. Profanity / toxicity detection
    if block_profanity {
        let profanity_matches: Vec<usize> = PROFANITY_SET.matches(&text).into_iter().collect();
        if !profanity_matches.is_empty() {
            let pattern_names: Vec<String> = profanity_matches
                .iter()
                .map(|i| format!("profanity_{}", i))
                .collect();
            risk_score = (risk_score + 0.7).min(1.0);
            matched_patterns.extend(pattern_names);
        }
    }

    // 5. Bias / discrimination detection
    if block_bias {
        let bias_matches: Vec<usize> = BIAS_SET.matches(&text).into_iter().collect();
        if !bias_matches.is_empty() {
            let pattern_names: Vec<String> =
                bias_matches.iter().map(|i| format!("bias_{}", i)).collect();
            risk_score = (risk_score + 0.7).min(1.0);
            matched_patterns.extend(pattern_names);
        }
    }

    // 6. Sensitive topics detection
    if block_sensitive_topics {
        let sensitive_matches: Vec<usize> =
            SENSITIVE_TOPIC_SET.matches(&text).into_iter().collect();
        if !sensitive_matches.is_empty() {
            let pattern_names: Vec<String> = sensitive_matches
                .iter()
                .map(|i| format!("sensitive_topic_{}", i))
                .collect();
            risk_score = (risk_score + 0.6).min(1.0);
            matched_patterns.extend(pattern_names);
        }
    }

    // 7. Gibberish / encoding smuggling detection
    if block_gibberish {
        let gibberish_matches: Vec<usize> = GIBBERISH_SET.matches(&text).into_iter().collect();
        if !gibberish_matches.is_empty() {
            let pattern_names: Vec<String> = gibberish_matches
                .iter()
                .map(|i| format!("gibberish_{}", i))
                .collect();
            risk_score = (risk_score + 0.5).min(1.0);
            matched_patterns.extend(pattern_names);
        }
    }

    // 8. Contact information detection
    if block_contact_info {
        let contact_matches: Vec<usize> = CONTACT_INFO_SET.matches(&text).into_iter().collect();
        if !contact_matches.is_empty() {
            let pattern_names: Vec<String> = contact_matches
                .iter()
                .map(|i| format!("contact_info_{}", i))
                .collect();
            risk_score = (risk_score + 0.5).min(1.0);
            matched_patterns.extend(pattern_names);
        }
    }

    // 9. IP / confidential leakage detection
    if block_ip_leakage {
        let ip_matches: Vec<usize> = IP_LEAKAGE_SET.matches(&text).into_iter().collect();
        if !ip_matches.is_empty() {
            let pattern_names: Vec<String> = ip_matches
                .iter()
                .map(|i| format!("ip_leakage_{}", i))
                .collect();
            risk_score = (risk_score + 0.6).min(1.0);
            matched_patterns.extend(pattern_names);
        }
    }

    // 10. Competitor mention detection (configurable names)
    if block_competitor_mention && !competitor_names.is_empty() {
        let text_lower = text.to_lowercase();
        for (i, name) in competitor_names.iter().enumerate() {
            if text_lower.contains(&name.to_lowercase()) {
                matched_patterns.push(format!("competitor_{}:{}", i, name));
                risk_score = (risk_score + 0.6).min(1.0);
            }
        }
    }

    // 11. Topic denylist — word-boundary aware (SEC 3A-4 FIX)
    // Previously used .contains() which matched subwords (e.g. "sex" matched "context").
    // Now uses \bterm\b to prevent false positives on common subword occurrences.
    // SEC: Uses cached regex compilation (same as custom patterns) to avoid recompilation
    // on every request and to apply size limits for ReDoS protection.
    {
        for topic in topic_denylist {
            let pattern = format!(r"(?i)\b{}\b", regex::escape(topic));
            if let Some(re) = compile_cached_guardrail(&pattern) {
                if re.is_match(&text) {
                    matched_patterns.push(format!("topic_deny:{}", topic));
                    risk_score = (risk_score + 0.6).min(1.0);
                }
            } else {
                // Fallback to contains() if pattern is somehow invalid
                if text.to_lowercase().contains(&topic.to_lowercase()) {
                    matched_patterns.push(format!("topic_deny:{}", topic));
                    risk_score = (risk_score + 0.6).min(1.0);
                }
            }
        }
    }

    // 12. Topic allowlist — if set, block anything NOT in the allowlist
    // FIX M-1: Use word-boundary matching (same as denylist) to prevent false
    // allows from substring matching (e.g., allowlist ["ai"] matching "main").
    // SEC: Uses cached regex compilation (same as custom patterns) for performance.
    if !topic_allowlist.is_empty() {
        let any_allowed = topic_allowlist.iter().any(|t| {
            let pattern = format!(r"(?i)\b{}\b", regex::escape(t));
            compile_cached_guardrail(&pattern)
                .map(|re| re.is_match(&text))
                .unwrap_or_else(|| text.to_lowercase().contains(&t.to_lowercase()))
        });
        if !any_allowed {
            matched_patterns.push("topic_allowlist_violation".to_string());
            risk_score = (risk_score + 0.6).min(1.0);
        }
    }

    // 13. Custom patterns
    // SEC: compile with size limit to prevent ReDoS from policy-authored patterns.
    // Cached per-thread to avoid recompilation on every request (same pattern as engine.rs).
    // Note: compile_cached_guardrail is defined above and shared with topic denylist/allowlist.
    {
        for (i, pattern) in custom_patterns.iter().enumerate() {
            if let Some(re) = compile_cached_guardrail(pattern) {
                if re.is_match(&text) {
                    matched_patterns.push(format!("custom_{}", i));
                    risk_score = (risk_score + 0.6).min(1.0);
                }
            }
        }
    }

    // 14. Content length check
    if max_content_length > 0 && text.len() > max_content_length as usize {
        matched_patterns.push(format!(
            "content_too_long:{}/{}",
            text.len(),
            max_content_length
        ));
        risk_score = (risk_score + 0.3).min(1.0);
    }

    // Apply threshold
    if risk_score >= risk_threshold && !matched_patterns.is_empty() {
        GuardrailResult::block(
            format!(
                "Request blocked by content filter (risk score: {:.2})",
                risk_score
            ),
            matched_patterns,
            risk_score,
        )
    } else {
        GuardrailResult {
            blocked: false,
            reason: None,
            matched_patterns,
            risk_score,
        }
    }
}

// ── Text Extraction ───────────────────────────────────────────

/// Extract all user-visible text from a request body.
/// Handles OpenAI chat format (`messages[].content`), tool calls, and raw string bodies.
fn extract_text_content(body: &Value) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(messages) = body.get("messages").and_then(|m| m.as_array()) {
        for msg in messages {
            // 1. Extract message content
            if let Some(content) = msg.get("content") {
                match content {
                    Value::String(s) => parts.push(s.clone()),
                    Value::Array(arr) => {
                        // Multimodal: [{type: "text", text: "..."}, ...]
                        for part in arr {
                            if part.get("type").and_then(|t| t.as_str()) == Some("text") {
                                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                                    parts.push(text.to_string());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            // 2. Extract tool_calls[].function.arguments (OpenAI format)
            if let Some(tool_calls) = msg.get("tool_calls").and_then(|t| t.as_array()) {
                for tc in tool_calls {
                    if let Some(args) = tc.pointer("/function/arguments").and_then(|a| a.as_str()) {
                        parts.push(args.to_string());
                    }
                }
            }

            // 3. Extract function_call.arguments (legacy format)
            if let Some(args) = msg.pointer("/function_call/arguments").and_then(|a| a.as_str()) {
                parts.push(args.to_string());
            }
        }
    }

    // 4. Extract tools[].function.description (tool definitions)
    if let Some(tools) = body.get("tools").and_then(|t| t.as_array()) {
        for tool in tools {
            if let Some(desc) = tool.pointer("/function/description").and_then(|d| d.as_str()) {
                parts.push(desc.to_string());
            }
        }
    }

    // Also handle raw text in `input` (embeddings) or `prompt` (completions)
    if let Some(input) = body.get("input").and_then(|v| v.as_str()) {
        parts.push(input.to_string());
    }
    if let Some(prompt) = body.get("prompt").and_then(|v| v.as_str()) {
        parts.push(prompt.to_string());
    }

    parts.join(" ")
}
