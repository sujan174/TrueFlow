mod bedrock;
mod error;
mod headers;
mod request;
mod response;
mod streaming;
mod url_rewrite;

#[cfg(test)]
mod tests;

// ── Public API re-exports ──────────────────────────────────────────────
pub(crate) use self::bedrock::decode_bedrock_event_stream;
pub(crate) use self::error::normalize_error_response;
pub(crate) use self::headers::inject_provider_headers;
pub(crate) use self::request::translate_request;
pub(crate) use self::response::translate_response;
pub(crate) use self::streaming::{
    openai_sse_chunk, translate_anthropic_sse_to_openai, translate_gemini_sse_to_openai,
};
pub(crate) use self::url_rewrite::rewrite_upstream_url;

/// Supported LLM providers for request/response translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    OpenAI,
    /// Azure OpenAI Service — same JSON format as OpenAI, different URL structure + api-key header.
    AzureOpenAI,
    Anthropic,
    Gemini,
    /// Groq — OpenAI-compatible API at api.groq.com
    Groq,
    /// Mistral AI — OpenAI-compatible API at api.mistral.ai
    Mistral,
    /// Together AI — OpenAI-compatible API at api.together.xyz
    TogetherAI,
    /// Cohere — Command-R models via api.cohere.com (OpenAI-compatible endpoint)
    Cohere,
    /// Ollama — local OpenAI-compatible server (default: http://localhost:11434)
    Ollama,
    /// Amazon Bedrock — Converse API with SigV4 auth + binary event stream for streaming
    Bedrock,
    Unknown,
}

impl Provider {
    /// Convert provider to lowercase string for cost calculation and logging.
    /// Returns the same format used by the pricing module's match statements.
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::OpenAI => "openai",
            Provider::AzureOpenAI => "azure",
            Provider::Anthropic => "anthropic",
            Provider::Gemini => "google",
            Provider::Groq => "groq",
            Provider::Mistral => "mistral",
            Provider::TogetherAI => "together",
            Provider::Cohere => "cohere",
            Provider::Ollama => "ollama",
            Provider::Bedrock => "bedrock",
            Provider::Unknown => "unknown",
        }
    }
}

/// Detect the provider from the model name or upstream URL.
///
/// Fast path: dispatch on the first ASCII byte of the model name (zero
/// allocation), then do a single case-insensitive prefix check.  Only falls
/// through to the URL scan when the model string is empty or unrecognised.
pub fn detect_provider(model: &str, upstream_url: &str) -> Provider {
    // ── Fast path: first-byte dispatch (no allocation) ───────────────────
    if let Some(first) = model.bytes().next() {
        match first.to_ascii_lowercase() {
            // 'a' → anthropic.claude-* (Bedrock), amazon.titan-* (Bedrock)
            b'a' => {
                if starts_with_ignore_ascii_case(model, "anthropic.") {
                    return Provider::Bedrock;
                }
                if starts_with_ignore_ascii_case(model, "amazon.") {
                    return Provider::Bedrock;
                }
            }
            // 'c' → claude-* (Anthropic), command-r* / command-* (Cohere), cohere.* (Bedrock)
            b'c' => {
                if starts_with_ignore_ascii_case(model, "claude") {
                    return Provider::Anthropic;
                }
                if starts_with_ignore_ascii_case(model, "command-r")
                    || starts_with_ignore_ascii_case(model, "command-")
                {
                    return Provider::Cohere;
                }
                if starts_with_ignore_ascii_case(model, "cohere.") {
                    return Provider::Bedrock;
                }
            }
            // 'g' → gemini-* or gpt-*, or slash-separated Together models (google/*)
            b'g' => {
                if starts_with_ignore_ascii_case(model, "gemini") {
                    return Provider::Gemini;
                }
                if starts_with_ignore_ascii_case(model, "gpt") {
                    return Provider::OpenAI;
                }
                if model.contains('/') {
                    return Provider::TogetherAI;
                }
            }
            // 'm' → meta.llama-* (Bedrock), mistral-* / mixtral-* (Mistral),
            //        meta-llama/* (Together — slash separator)
            b'm' => {
                if starts_with_ignore_ascii_case(model, "meta.") {
                    return Provider::Bedrock;
                }
                if starts_with_ignore_ascii_case(model, "mistral-")
                    || starts_with_ignore_ascii_case(model, "mixtral-")
                {
                    return Provider::Mistral;
                }
                // Together AI: slash-separated model IDs like meta-llama/Llama-3-70b
                if starts_with_ignore_ascii_case(model, "meta-llama/")
                    || starts_with_ignore_ascii_case(model, "mistralai/")
                {
                    return Provider::TogetherAI;
                }
            }
            // 'o' → o1-* / o3-* / o4-* (HIGH-7: explicit prefix check to avoid false positives)
            b'o' => {
                // Use explicit prefix check to avoid matching other models that happen
                // to start with 'o' followed by '1', '3', or '4'
                if starts_with_ignore_ascii_case(model, "o1-")
                    || starts_with_ignore_ascii_case(model, "o3-")
                    || starts_with_ignore_ascii_case(model, "o4-")
                {
                    return Provider::OpenAI;
                }
            }
            // 'q' → Qwen/* (Together)
            b'q' if starts_with_ignore_ascii_case(model, "Qwen/")
                || starts_with_ignore_ascii_case(model, "qwen/") =>
            {
                return Provider::TogetherAI;
            }
            // 't' → text-* / tts-*
            b't' => {
                if starts_with_ignore_ascii_case(model, "text-")
                    || starts_with_ignore_ascii_case(model, "tts")
                {
                    return Provider::OpenAI;
                }
            }
            // 'd' → dall-e-*, deepseek/* (Together)
            b'd' => {
                if starts_with_ignore_ascii_case(model, "dall-e") {
                    return Provider::OpenAI;
                }
                if starts_with_ignore_ascii_case(model, "deepseek/") {
                    return Provider::TogetherAI;
                }
            }
            // 'w' → whisper-*
            b'w' if starts_with_ignore_ascii_case(model, "whisper") => {
                return Provider::OpenAI;
            }
            _ => {
                // Together AI: any model with a '/' separator that we haven't matched above
                // (e.g., "google/gemma-2-9b-it", "NousResearch/Hermes-2")
                if model.contains('/') {
                    return Provider::TogetherAI;
                }
            }
        }
    }

    // ── URL-based fallback (only reached for empty/unknown model names) ──
    let url_lower = upstream_url.to_lowercase();
    if url_lower.contains("anthropic") && !url_lower.contains("bedrock") {
        // Task 36: Record URL-based fallback
        crate::middleware::metrics::record_provider_derivation_fallback("anthropic");
        return Provider::Anthropic;
    }
    if url_lower.contains("generativelanguage.googleapis.com")
        || url_lower.contains("aiplatform.googleapis.com")
    {
        crate::middleware::metrics::record_provider_derivation_fallback("google");
        return Provider::Gemini;
    }
    // Azure OpenAI: detect by endpoint URL patterns
    if url_lower.contains("azure.com") && url_lower.contains("openai")
        || url_lower.contains(".openai.azure.com")
        || url_lower.contains("azure-api.net")
    {
        crate::middleware::metrics::record_provider_derivation_fallback("azure");
        return Provider::AzureOpenAI;
    }
    // Bedrock: region-specific endpoints
    if url_lower.contains("bedrock-runtime") || url_lower.contains("bedrock") {
        crate::middleware::metrics::record_provider_derivation_fallback("bedrock");
        return Provider::Bedrock;
    }
    if url_lower.contains("groq.com") {
        crate::middleware::metrics::record_provider_derivation_fallback("groq");
        return Provider::Groq;
    }
    if url_lower.contains("mistral.ai") {
        crate::middleware::metrics::record_provider_derivation_fallback("mistral");
        return Provider::Mistral;
    }
    if url_lower.contains("together.xyz") || url_lower.contains("together.ai") {
        crate::middleware::metrics::record_provider_derivation_fallback("together");
        return Provider::TogetherAI;
    }
    if url_lower.contains("cohere.com") || url_lower.contains("cohere.ai") {
        crate::middleware::metrics::record_provider_derivation_fallback("cohere");
        return Provider::Cohere;
    }
    if url_lower.contains("localhost:11434")
        || url_lower.contains("ollama")
        || url_lower.contains(":11434")
    {
        crate::middleware::metrics::record_provider_derivation_fallback("ollama");
        return Provider::Ollama;
    }
    if url_lower.contains("openai") {
        crate::middleware::metrics::record_provider_derivation_fallback("openai");
        return Provider::OpenAI;
    }

    Provider::Unknown
}

/// Case-insensitive ASCII prefix check without allocating.
#[inline(always)]
fn starts_with_ignore_ascii_case(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len() && s.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}
