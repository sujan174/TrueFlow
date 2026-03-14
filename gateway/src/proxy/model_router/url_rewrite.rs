use super::Provider;

/// Sanitize a model name for safe inclusion in a URL path segment.
///
/// This prevents URL injection attacks where malicious model names could:
/// - Inject query parameters (e.g., "model?api_key=stolen")
/// - Perform path traversal (e.g., "../../../admin")
/// - Inject URL fragments (e.g., "model#fragment")
/// - Include control characters that affect downstream processing
///
/// The function validates that the model name doesn't contain path traversal
/// sequences or control characters, then URL-encodes if needed.
fn sanitize_model_for_url(model: &str) -> String {
    // Check for path traversal attempts
    if model.contains("..") {
        tracing::warn!(
            model = %model,
            "Model name contains path traversal sequence, rejecting"
        );
        // Return empty string to cause a safe failure downstream
        return String::new();
    }

    // Check for characters that could enable injection attacks
    // If any dangerous characters are present, URL-encode the entire model name
    let needs_encoding = model.chars().any(|c| {
        c == '?' || c == '#' || c == '\\' || c.is_control() || c.is_whitespace()
    });

    if needs_encoding {
        urlencoding::encode(model).into_owned()
    } else {
        model.to_string()
    }
}

pub(crate) fn rewrite_upstream_url(
    provider: Provider,
    base_url: &str,
    model: &str,
    is_streaming: bool,
) -> String {
    // Strip the proxy path if the router attached it (e.g. TrueFlow added /v1/chat/completions)
    let sanitized_base = base_url
        .strip_suffix("/v1/chat/completions")
        .unwrap_or(base_url)
        .trim_end_matches('/');

    match provider {
        Provider::Gemini => {
            // Gemini uses different endpoints for streaming vs non-streaming
            let method = if is_streaming {
                "streamGenerateContent"
            } else {
                "generateContent"
            };
            let safe_model = sanitize_model_for_url(model);
            format!("{}/v1beta/models/{}:{}", sanitized_base, safe_model, method)
        }
        Provider::Anthropic => {
            // Anthropic API: POST https://api.anthropic.com/v1/messages
            format!("{}/v1/messages", sanitized_base)
        }
        Provider::AzureOpenAI => {
            // Azure OpenAI: {endpoint}/openai/deployments/{deployment}/chat/completions
            if sanitized_base
                .to_lowercase()
                .contains("/openai/deployments/")
            {
                // Already has the deployment path — ensure api-version is present
                if !sanitized_base.contains("api-version") {
                    format!("{}?api-version=2024-05-01-preview", sanitized_base)
                } else {
                    sanitized_base.to_string()
                }
            } else {
                let safe_model = sanitize_model_for_url(model);
                format!(
                    "{}/openai/deployments/{}/chat/completions?api-version=2024-05-01-preview",
                    sanitized_base, safe_model
                )
            }
        }
        Provider::Bedrock => {
            // Bedrock Converse API: {endpoint}/model/{modelId}/converse or converse-stream
            let action = if is_streaming {
                "converse-stream"
            } else {
                "converse"
            };
            if sanitized_base.contains("/model/") {
                // Already has model path — just ensure correct action
                sanitized_base.to_string()
            } else {
                let safe_model = sanitize_model_for_url(model);
                format!("{}/model/{}/{}", sanitized_base, safe_model, action)
            }
        }
        Provider::Ollama => {
            // Ollama: http://localhost:11434/api/chat (or /v1/chat/completions for OpenAI compat mode)
            if sanitized_base.contains("/v1") || sanitized_base.contains("/api/") {
                sanitized_base.to_string()
            } else {
                format!("{}/v1/chat/completions", sanitized_base)
            }
        }
        // Groq, Mistral, Together, Cohere all use standard /v1/chat/completions via their base URLs
        Provider::Groq | Provider::Mistral | Provider::TogetherAI | Provider::Cohere => {
            if sanitized_base.contains("/v1") {
                sanitized_base.to_string()
            } else {
                format!("{}/v1/chat/completions", sanitized_base)
            }
        }
        _ => base_url.to_string(),
    }
}
