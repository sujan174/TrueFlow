use serde_json::json;

use super::Provider;

pub(crate) fn normalize_error_response(
    provider: Provider,
    body: &[u8],
) -> Option<serde_json::Value> {
    match provider {
        // Azure OpenAI, Groq, Mistral, Together, Cohere, Ollama all return OpenAI-compatible errors
        Provider::OpenAI
        | Provider::AzureOpenAI
        | Provider::Groq
        | Provider::Mistral
        | Provider::TogetherAI
        | Provider::Cohere
        | Provider::Ollama
        | Provider::Unknown => None,
        Provider::Anthropic => {
            // Anthropic error format:
            // {"type":"error","error":{"type":"invalid_request_error","message":"..."}}
            let json: serde_json::Value = serde_json::from_slice(body).ok()?;
            let err = json.get("error")?;
            let message = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            let err_type = err
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("api_error");
            tracing::debug!(
                provider = "anthropic",
                err_type,
                message,
                "normalizing Anthropic error to OpenAI format"
            );
            Some(json!({
                "error": {
                    "message": message,
                    "type": err_type,
                    "param": null,
                    "code": null
                }
            }))
        }
        Provider::Gemini => {
            // Gemini error format (wrapped in array):
            // [{"error":{"code":400,"message":"...","status":"INVALID_ARGUMENT"}}]
            // OR: {"error":{"code":400,"message":"...","status":"..."}}
            let json: serde_json::Value = serde_json::from_slice(body).ok()?;
            let err_obj = if json.is_array() {
                json.as_array()?.first()?.get("error")?
            } else {
                json.get("error")?
            };
            let message = err_obj
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            let status = err_obj
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("api_error");
            let http_code = err_obj.get("code").and_then(|c| c.as_u64()).unwrap_or(500) as u16;

            // Map Gemini status codes to OpenAI-compatible error types
            let (error_type, openai_code): (&str, &str) = match status {
                // Rate limiting (429)
                "RESOURCE_EXHAUSTED" => ("rate_limit_exceeded", "rate_limit_exceeded"),
                // Server errors (500)
                "INTERNAL" => ("server_error", "server_error"),
                // Service unavailable (503)
                "UNAVAILABLE" => ("server_error", "server_error"),
                // Bad request (400)
                "INVALID_ARGUMENT" => ("invalid_request_error", "invalid_request_error"),
                // Not found (404)
                "NOT_FOUND" => ("not_found_error", "not_found_error"),
                // Permission denied (403)
                "PERMISSION_DENIED" => ("permission_denied", "permission_denied"),
                // Authentication error (401)
                "UNAUTHENTICATED" => ("authentication_error", "authentication_error"),
                // Default: unknown error type
                _ => ("api_error", "api_error"),
            };

            tracing::debug!(
                provider = "gemini",
                status,
                http_code,
                error_type,
                message,
                "normalizing Gemini error to OpenAI format"
            );
            Some(json!({
                "error": {
                    "message": message,
                    "type": error_type,
                    "param": null,
                    "code": openai_code
                }
            }))
        }
        Provider::Bedrock => {
            // Bedrock error format:
            // {"message":"...","__type":"ValidationException"}
            // OR: {"Message":"...","__type":"..."}
            let json: serde_json::Value = serde_json::from_slice(body).ok()?;
            let message = json
                .get("message")
                .or_else(|| json.get("Message"))
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            let err_type = json
                .get("__type")
                .and_then(|t| t.as_str())
                .unwrap_or("api_error");
            // Convert AWS exception type to snake_case
            let normalized_type = err_type
                .rsplit_once('#')
                .map(|(_, s)| s)
                .unwrap_or(err_type)
                .replace("Exception", "")
                .chars()
                .enumerate()
                .map(|(i, c)| {
                    if c.is_uppercase() && i > 0 {
                        format!("_{}", c.to_lowercase())
                    } else {
                        c.to_lowercase().to_string()
                    }
                })
                .collect::<String>()
                .trim_start_matches('_')
                .to_string();
            tracing::debug!(
                provider = "bedrock",
                err_type,
                message,
                "normalizing Bedrock error to OpenAI format"
            );
            Some(json!({
                "error": {
                    "message": message,
                    "type": normalized_type,
                    "param": null,
                    "code": err_type
                }
            }))
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// OpenAI → Bedrock (Converse API)
// ═══════════════════════════════════════════════════════════════
