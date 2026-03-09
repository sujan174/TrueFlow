use serde_json::Value;

use super::operators::glob_match;
use crate::models::policy::Action;

pub(super) fn action_name(action: &Action) -> &'static str {
    match action {
        Action::Allow => "allow",
        Action::Deny { .. } => "deny",
        Action::RequireApproval { .. } => "require_approval",
        Action::RateLimit { .. } => "rate_limit",
        Action::Throttle { .. } => "throttle",
        Action::Redact { .. } => "redact",
        Action::Transform { .. } => "transform",
        Action::Override { .. } => "override",
        Action::Log { .. } => "log",
        Action::Tag { .. } => "tag",
        Action::Webhook { .. } => "webhook",
        Action::ContentFilter { .. } => "content_filter",
        Action::Split { .. } => "split",
        Action::DynamicRoute { .. } => "dynamic_route",
        Action::ValidateSchema { .. } => "validate_schema",
        Action::ConditionalRoute { .. } => "conditional_route",
        Action::ExternalGuardrail { .. } => "external_guardrail",
        Action::ToolScope { .. } => "tool_scope",
    }
}

// ── Tool-Level RBAC ──────────────────────────────────────────

/// Extract tool function names from a request body.
///
/// Supports all major provider formats:
/// - OpenAI:    `tools[].function.name` and `tool_choice.function.name`
/// - Anthropic: `tools[].name`
/// - Gemini:    `tools[].function_declarations[].name`
pub fn extract_tool_names(body: Option<&Value>) -> Vec<String> {
    let Some(body) = body else { return vec![] };
    let mut names = Vec::new();

    // OpenAI: tools[].function.name
    if let Some(Value::Array(tools)) = body.get("tools") {
        for tool in tools {
            if let Some(name) = tool.pointer("/function/name").and_then(|v| v.as_str()) {
                names.push(name.to_string());
            }
            // Anthropic: tools[].name (direct)
            if let Some(name) = tool.get("name").and_then(|v| v.as_str()) {
                if !names.contains(&name.to_string()) {
                    names.push(name.to_string());
                }
            }
            // Gemini: tools[].function_declarations[].name
            if let Some(Value::Array(decls)) = tool.get("function_declarations") {
                for decl in decls {
                    if let Some(name) = decl.get("name").and_then(|v| v.as_str()) {
                        if !names.contains(&name.to_string()) {
                            names.push(name.to_string());
                        }
                    }
                }
            }
        }
    }

    // OpenAI: tool_choice.function.name (forced tool selection)
    if let Some(name) = body
        .pointer("/tool_choice/function/name")
        .and_then(|v| v.as_str())
    {
        if !names.contains(&name.to_string()) {
            names.push(name.to_string());
        }
    }

    // OpenAI: tool_calls[].function.name (in responses/streaming)
    if let Some(Value::Array(calls)) = body.get("tool_calls") {
        for call in calls {
            if let Some(name) = call.pointer("/function/name").and_then(|v| v.as_str()) {
                if !names.contains(&name.to_string()) {
                    names.push(name.to_string());
                }
            }
        }
    }

    names
}

/// Evaluate tool scope policy against extracted tool names.
///
/// Returns `Ok(())` if all tools are authorized, or `Err(deny_message)` with
/// the specific tool name and reason on violation.
pub fn evaluate_tool_scope(
    tool_names: &[String],
    allowed_tools: &[String],
    blocked_tools: &[String],
    deny_message: &str,
) -> Result<(), String> {
    // Check blocked tools first (explicit deny takes priority)
    for name in tool_names {
        if blocked_tools
            .iter()
            .any(|b| b == name || glob_match(b, name))
        {
            return Err(format!("{}: tool '{}' is blocked", deny_message, name));
        }
    }

    // If allowed_tools is non-empty, enforce whitelist
    if !allowed_tools.is_empty() {
        for name in tool_names {
            if !allowed_tools
                .iter()
                .any(|a| a == name || glob_match(a, name))
            {
                return Err(format!(
                    "{}: tool '{}' is not in the allowed list",
                    deny_message, name
                ));
            }
        }
    }

    Ok(())
}

// ── Tests ────────────────────────────────────────────────────
