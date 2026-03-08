use super::Provider;

pub(crate) fn rewrite_upstream_url(provider: Provider, base_url: &str, model: &str, is_streaming: bool) -> String {
    // Strip the proxy path if the router attached it (e.g. TrueFlow added /v1/chat/completions)
    let sanitized_base = base_url
        .strip_suffix("/v1/chat/completions")
        .unwrap_or(base_url)
        .trim_end_matches('/');

    match provider {
        Provider::Gemini => {
            // Gemini uses different endpoints for streaming vs non-streaming
            let method = if is_streaming { "streamGenerateContent" } else { "generateContent" };
            format!("{}/v1beta/models/{}:{}", sanitized_base, model, method)
        }
        Provider::Anthropic => {
            // Anthropic API: POST https://api.anthropic.com/v1/messages
            format!("{}/v1/messages", sanitized_base)
        }
        Provider::AzureOpenAI => {
            // Azure OpenAI: {endpoint}/openai/deployments/{deployment}/chat/completions
            if sanitized_base.to_lowercase().contains("/openai/deployments/") {
                // Already has the deployment path — ensure api-version is present
                if !sanitized_base.contains("api-version") {
                    format!("{}?api-version=2024-05-01-preview", sanitized_base)
                } else {
                    sanitized_base.to_string()
                }
            } else {
                format!("{}/openai/deployments/{}/chat/completions?api-version=2024-05-01-preview", sanitized_base, model)
            }
        }
        Provider::Bedrock => {
            // Bedrock Converse API: {endpoint}/model/{modelId}/converse or converse-stream
            let action = if is_streaming { "converse-stream" } else { "converse" };
            if sanitized_base.contains("/model/") {
                // Already has model path — just ensure correct action
                sanitized_base.to_string()
            } else {
                format!("{}/model/{}/{}", sanitized_base, model, action)
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

