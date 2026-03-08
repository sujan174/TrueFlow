use super::Provider;

pub(crate) fn inject_provider_headers(
    provider: Provider,
    headers: &mut reqwest::header::HeaderMap,
    is_streaming: bool,
) {
    use reqwest::header::{HeaderName, HeaderValue};
    match provider {
        Provider::Anthropic => {
            // Required on every Anthropic request (entry().or_insert so policy-set header wins)
            headers
                .entry(HeaderName::from_static("anthropic-version"))
                .or_insert(HeaderValue::from_static("2023-06-01"));
            // Streaming requires explicit Accept header
            if is_streaming {
                headers
                    .entry(reqwest::header::ACCEPT)
                    .or_insert(HeaderValue::from_static("text/event-stream"));
            }
        }
        Provider::Gemini => {
            // Gemini SSE streaming needs Accept: text/event-stream
            if is_streaming {
                headers
                    .entry(reqwest::header::ACCEPT)
                    .or_insert(HeaderValue::from_static("text/event-stream"));
            }
        }
        // Groq, Mistral, Together, Cohere — Accept: text/event-stream for streaming
        Provider::Groq | Provider::Mistral | Provider::TogetherAI | Provider::Cohere => {
            if is_streaming {
                headers
                    .entry(reqwest::header::ACCEPT)
                    .or_insert(HeaderValue::from_static("text/event-stream"));
            }
        }
        Provider::Bedrock => {
            // Bedrock requires Accept for streaming: application/vnd.amazon.eventstream
            if is_streaming {
                headers
                    .entry(reqwest::header::ACCEPT)
                    .or_insert(HeaderValue::from_static("application/vnd.amazon.eventstream"));
            }
            // Content-Type for Converse API is always application/json
            headers
                .entry(reqwest::header::CONTENT_TYPE)
                .or_insert(HeaderValue::from_static("application/json"));
        }
        _ => {}
    }
}
