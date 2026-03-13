use rust_decimal::Decimal;
use serde_json::Value;
use std::str::FromStr;

pub fn extract_usage(_upstream_url: &str, body: &[u8]) -> anyhow::Result<Option<(u32, u32)>> {
    // Try to parse body as JSON
    let json: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return Ok(None), // Not JSON, or empty
    };

    // Logical check for standard "usage" object (OpenAI / Anthropic / Mistral / Bedrock)
    if let Some(usage) = json.get("usage") {
        let input = usage
            .get("prompt_tokens")
            .or_else(|| usage.get("input_tokens"))
            // FIX H-4: Bedrock Converse API uses camelCase field names
            .or_else(|| usage.get("inputTokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let output = usage
            .get("completion_tokens")
            .or_else(|| usage.get("output_tokens"))
            // FIX H-4: Bedrock Converse API uses camelCase field names
            .or_else(|| usage.get("outputTokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        if input > 0 || output > 0 {
            return Ok(Some((input, output)));
        }
    }

    // Gemini: usageMetadata
    if let Some(meta) = json.get("usageMetadata") {
        let input = meta
            .get("promptTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let output = meta
            .get("candidatesTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        // FIX H-5: Gemini's promptTokenCount already INCLUDES cachedContentTokenCount.
        // Previously we did input.saturating_add(cached) which double-counted cached tokens.
        if input > 0 || output > 0 {
            return Ok(Some((input, output)));
        }
    }

    // No usage object found - log warning with response shape for debugging
    tracing::warn!(
        response_keys = ?json.as_object().map(|o| o.keys().collect::<Vec<_>>()),
        "extract_usage: No usage or usageMetadata field found in response"
    );

    Ok(None)
}

pub fn extract_model(body: &[u8]) -> Option<String> {
    let json: Value = serde_json::from_slice(body).ok()?;
    json.get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelPricing {
    pub input_cost_per_m: Decimal,
    pub output_cost_per_m: Decimal,
}

/// Hardcoded fallback pricing table (USD per 1M tokens).
/// Used when the DB-backed cache is empty (e.g., before first load or on DB error).
///
/// IMPORTANT: More-specific patterns must come before less-specific ones.
/// E.g. "gpt-4o-mini" must precede "gpt-4o" because `contains` matches both.
/// Prices sourced from provider pricing pages as of 2025-02.
pub fn get_model_pricing_fallback(provider: &str, model: &str) -> ModelPricing {
    let zero = Decimal::ZERO;
    let d = |s: &str| Decimal::from_str(s).unwrap();

    match (provider, model) {
        // ── OpenAI ────────────────────────────────────────────────
        // BUG-3 FIX: gpt-4o-mini MUST come before gpt-4o (contains match)
        ("openai", m) if m.contains("gpt-4o-mini") => ModelPricing {
            input_cost_per_m: d("0.15"),
            output_cost_per_m: d("0.60"),
        },
        ("openai", m) if m.contains("gpt-4o") => ModelPricing {
            input_cost_per_m: d("2.50"),
            output_cost_per_m: d("10.00"),
        },
        ("openai", m) if m.contains("o3-mini") => ModelPricing {
            input_cost_per_m: d("1.10"),
            output_cost_per_m: d("4.40"),
        },
        ("openai", m) if m.contains("o3") => ModelPricing {
            input_cost_per_m: d("10.00"),
            output_cost_per_m: d("40.00"),
        },
        ("openai", m) if m.contains("o1-mini") => ModelPricing {
            input_cost_per_m: d("3.00"),
            output_cost_per_m: d("12.00"),
        },
        ("openai", m) if m.contains("o1") => ModelPricing {
            input_cost_per_m: d("15.00"),
            output_cost_per_m: d("60.00"),
        },
        ("openai", m) if m.contains("gpt-4-turbo") => ModelPricing {
            input_cost_per_m: d("10.00"),
            output_cost_per_m: d("30.00"),
        },
        ("openai", m) if m.contains("gpt-4") => ModelPricing {
            input_cost_per_m: d("30.00"),
            output_cost_per_m: d("60.00"),
        },
        ("openai", m) if m.contains("gpt-3.5-turbo") => ModelPricing {
            input_cost_per_m: d("0.50"),
            output_cost_per_m: d("1.50"),
        },
        // Embeddings (output cost is zero by convention)
        ("openai", m) if m.contains("text-embedding-3-small") => ModelPricing {
            input_cost_per_m: d("0.02"),
            output_cost_per_m: zero,
        },
        ("openai", m) if m.contains("text-embedding-3-large") => ModelPricing {
            input_cost_per_m: d("0.13"),
            output_cost_per_m: zero,
        },
        ("openai", m) if m.contains("text-embedding") => ModelPricing {
            input_cost_per_m: d("0.10"),
            output_cost_per_m: zero,
        },

        // ── Anthropic ─────────────────────────────────────────────
        ("anthropic", m) if m.contains("claude-haiku-4") => ModelPricing {
            input_cost_per_m: d("0.25"),
            output_cost_per_m: d("1.25"),
        },
        ("anthropic", m) if m.contains("claude-sonnet-4") => ModelPricing {
            input_cost_per_m: d("3.00"),
            output_cost_per_m: d("15.00"),
        },
        ("anthropic", m) if m.contains("claude-opus-4") => ModelPricing {
            input_cost_per_m: d("15.00"),
            output_cost_per_m: d("75.00"),
        },
        ("anthropic", m) if m.contains("claude-3-5-haiku") => ModelPricing {
            input_cost_per_m: d("0.80"),
            output_cost_per_m: d("4.00"),
        },
        ("anthropic", m) if m.contains("claude-3-5-sonnet") => ModelPricing {
            input_cost_per_m: d("3.00"),
            output_cost_per_m: d("15.00"),
        },
        ("anthropic", m) if m.contains("claude-3-7-sonnet") => ModelPricing {
            input_cost_per_m: d("3.00"),
            output_cost_per_m: d("15.00"),
        },
        ("anthropic", m) if m.contains("claude-3-opus") => ModelPricing {
            input_cost_per_m: d("15.00"),
            output_cost_per_m: d("75.00"),
        },
        ("anthropic", m) if m.contains("claude-3-haiku") => ModelPricing {
            input_cost_per_m: d("0.25"),
            output_cost_per_m: d("1.25"),
        },

        // ── Google / Gemini ───────────────────────────────────────
        ("google", m) if m.contains("gemini-2.0-flash") => ModelPricing {
            input_cost_per_m: d("0.10"),
            output_cost_per_m: d("0.40"),
        },
        ("google", m) if m.contains("gemini-1.5-flash") => ModelPricing {
            input_cost_per_m: d("0.075"),
            output_cost_per_m: d("0.30"),
        },
        ("google", m) if m.contains("gemini-1.5-pro") => ModelPricing {
            input_cost_per_m: d("1.25"),
            output_cost_per_m: d("5.00"),
        },
        ("google", m) if m.contains("gemini-2.5-pro") => ModelPricing {
            input_cost_per_m: d("1.25"),
            output_cost_per_m: d("10.00"),
        },
        ("google", m) if m.contains("gemini") => ModelPricing {
            input_cost_per_m: d("0.50"),
            output_cost_per_m: d("1.50"),
        },

        // ── Mistral ───────────────────────────────────────────────
        ("mistral", m) if m.contains("mistral-large") => ModelPricing {
            input_cost_per_m: d("2.00"),
            output_cost_per_m: d("6.00"),
        },
        ("mistral", m) if m.contains("mistral-small") => ModelPricing {
            input_cost_per_m: d("0.20"),
            output_cost_per_m: d("0.60"),
        },
        ("mistral", m) if m.contains("codestral") => ModelPricing {
            input_cost_per_m: d("0.30"),
            output_cost_per_m: d("0.90"),
        },

        // ── Meta / Llama (via any provider) ───────────────────────
        (_, m) if m.contains("llama-3.1-405b") => ModelPricing {
            input_cost_per_m: d("3.00"),
            output_cost_per_m: d("3.00"),
        },
        (_, m) if m.contains("llama-3.1-70b") => ModelPricing {
            input_cost_per_m: d("0.88"),
            output_cost_per_m: d("0.88"),
        },
        (_, m) if m.contains("llama-3.1-8b") => ModelPricing {
            input_cost_per_m: d("0.05"),
            output_cost_per_m: d("0.08"),
        },

        // ── DeepSeek ──────────────────────────────────────────────
        (_, m) if m.contains("deepseek-chat") || m.contains("deepseek-v3") => ModelPricing {
            input_cost_per_m: d("0.27"),
            output_cost_per_m: d("1.10"),
        },
        (_, m) if m.contains("deepseek-reasoner") || m.contains("deepseek-r1") => ModelPricing {
            input_cost_per_m: d("0.55"),
            output_cost_per_m: d("2.19"),
        },

        // ── Groq (hosted inference) ──────────────────────────────
        ("groq", m) if m.contains("llama-3.1-70b") => ModelPricing {
            input_cost_per_m: d("0.59"),
            output_cost_per_m: d("0.79"),
        },
        ("groq", m) if m.contains("llama-3.1-8b") => ModelPricing {
            input_cost_per_m: d("0.05"),
            output_cost_per_m: d("0.08"),
        },
        ("groq", m) if m.contains("mixtral-8x7b") => ModelPricing {
            input_cost_per_m: d("0.24"),
            output_cost_per_m: d("0.24"),
        },
        ("groq", m) if m.contains("gemma") => ModelPricing {
            input_cost_per_m: d("0.15"),
            output_cost_per_m: d("0.15"),
        },

        // ── Cohere ───────────────────────────────────────────────
        ("cohere", m) if m.contains("command-r-plus") => ModelPricing {
            input_cost_per_m: d("2.50"),
            output_cost_per_m: d("10.00"),
        },
        ("cohere", m) if m.contains("command-r") => ModelPricing {
            input_cost_per_m: d("0.15"),
            output_cost_per_m: d("0.60"),
        },

        // ── Together AI ──────────────────────────────────────────
        ("together", m) if m.contains("llama-3.1-405b") => ModelPricing {
            input_cost_per_m: d("3.50"),
            output_cost_per_m: d("3.50"),
        },
        ("together", m) if m.contains("llama-3.1-70b") => ModelPricing {
            input_cost_per_m: d("0.88"),
            output_cost_per_m: d("0.88"),
        },
        ("together", m) if m.contains("llama-3.1-8b") => ModelPricing {
            input_cost_per_m: d("0.18"),
            output_cost_per_m: d("0.18"),
        },

        // ── Bedrock (facade — map to underlying model pricing) ───
        // Bedrock models use provider-prefixed names like anthropic.claude-*
        ("bedrock", m) if m.contains("claude-3-5-sonnet") || m.contains("claude-3-7-sonnet") => {
            ModelPricing {
                input_cost_per_m: d("3.00"),
                output_cost_per_m: d("15.00"),
            }
        }
        ("bedrock", m) if m.contains("claude-3-5-haiku") => ModelPricing {
            input_cost_per_m: d("0.80"),
            output_cost_per_m: d("4.00"),
        },
        ("bedrock", m) if m.contains("claude-3-opus") => ModelPricing {
            input_cost_per_m: d("15.00"),
            output_cost_per_m: d("75.00"),
        },
        ("bedrock", m) if m.contains("claude-3-haiku") => ModelPricing {
            input_cost_per_m: d("0.25"),
            output_cost_per_m: d("1.25"),
        },
        ("bedrock", m) if m.contains("llama") => ModelPricing {
            input_cost_per_m: d("0.88"),
            output_cost_per_m: d("0.88"),
        },
        ("bedrock", m) if m.contains("titan") => ModelPricing {
            input_cost_per_m: d("0.80"),
            output_cost_per_m: d("1.00"),
        },

        // 5A-2 FIX: Unknown models get a non-zero default to prevent silent
        // under-counting. $1.00/$3.00 per 1M is a conservative mid-range estimate.
        // Operators should add proper pricing via the DB pricing cache; this
        // warning log alerts them to missing model entries.
        _ => {
            tracing::warn!(
                provider = %provider,
                model = %model,
                "Unknown model — using fallback pricing ($1.00/$3.00 per 1M). \
                 Add this model to the pricing table to avoid inaccurate billing."
            );
            ModelPricing {
                input_cost_per_m: d("1.00"),
                output_cost_per_m: d("3.00"),
            }
        }
    }
}

/// Calculate cost using the DB-backed pricing cache.
/// Falls back to hardcoded table if the cache is empty.
///
/// This is the async version used by the proxy handler.
pub async fn calculate_cost_with_cache(
    pricing: &crate::models::pricing_cache::PricingCache,
    provider: &str,
    model: &str,
    input_tokens: u32,
    output_tokens: u32,
) -> Decimal {
    let one_million = Decimal::from(1_000_000);

    let (input_per_m, output_per_m) = if let Some(p) = pricing.lookup(provider, model).await {
        p
    } else {
        // Cache miss — fall back to hardcoded table
        let fallback = get_model_pricing_fallback(provider, model);
        (fallback.input_cost_per_m, fallback.output_cost_per_m)
    };

    let input_cost = (Decimal::from(input_tokens) / one_million) * input_per_m;
    let output_cost = (Decimal::from(output_tokens) / one_million) * output_per_m;
    input_cost + output_cost
}

/// Synchronous version kept for backwards compatibility with non-async call sites.
/// Uses only the hardcoded fallback table.
#[allow(dead_code)]
pub fn calculate_cost(
    provider: &str,
    model: &str,
    input_tokens: u32,
    output_tokens: u32,
) -> Decimal {
    let pricing = get_model_pricing_fallback(provider, model);
    let one_million = Decimal::from(1_000_000);

    let input_cost = (Decimal::from(input_tokens) / one_million) * pricing.input_cost_per_m;
    let output_cost = (Decimal::from(output_tokens) / one_million) * pricing.output_cost_per_m;

    input_cost + output_cost
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Pricing match-order tests (BUG-3 regression) ──────────

    #[test]
    fn test_gpt4o_mini_not_overcharged() {
        // BUG-3: gpt-4o-mini must match its own rule, not gpt-4o's
        let pricing = get_model_pricing_fallback("openai", "gpt-4o-mini-2024-07-18");
        assert_eq!(pricing.input_cost_per_m, Decimal::from_str("0.15").unwrap());
        assert_eq!(
            pricing.output_cost_per_m,
            Decimal::from_str("0.60").unwrap()
        );
    }

    #[test]
    fn test_gpt4o_cost() {
        // gpt-4o: $2.50/$10.00 per 1M
        let cost = calculate_cost("openai", "gpt-4o-2024-08-06", 1_000_000, 1_000_000);
        assert_eq!(cost, Decimal::from_str("12.50").unwrap());
    }

    #[test]
    fn test_o1_mini_before_o1() {
        // Same pattern-ordering issue: o1-mini must match before o1
        let p = get_model_pricing_fallback("openai", "o1-mini-2024-09-12");
        assert_eq!(p.input_cost_per_m, Decimal::from_str("3.00").unwrap());
    }

    #[test]
    fn test_o3_mini_before_o3() {
        let p = get_model_pricing_fallback("openai", "o3-mini-2025-01-31");
        assert_eq!(p.input_cost_per_m, Decimal::from_str("1.10").unwrap());
    }

    // ── Anthropic ────────────────────────────────────────────

    #[test]
    fn test_sonnet_cost() {
        // claude-3-5-sonnet: $3/$15 per 1M → 1M each = $18
        let cost = calculate_cost(
            "anthropic",
            "claude-3-5-sonnet-20240620",
            1_000_000,
            1_000_000,
        );
        assert_eq!(cost, Decimal::from_str("18.00").unwrap());
    }

    #[test]
    fn test_claude_3_5_haiku_cost() {
        let p = get_model_pricing_fallback("anthropic", "claude-3-5-haiku-20241022");
        assert_eq!(p.input_cost_per_m, Decimal::from_str("0.80").unwrap());
    }

    // ── Gemini ───────────────────────────────────────────────

    #[test]
    fn test_gemini_flash_cost() {
        let p = get_model_pricing_fallback("google", "gemini-2.0-flash");
        assert_eq!(p.input_cost_per_m, Decimal::from_str("0.10").unwrap());
        assert_eq!(p.output_cost_per_m, Decimal::from_str("0.40").unwrap());
    }

    #[test]
    fn test_gemini_pro_cost() {
        let p = get_model_pricing_fallback("google", "gemini-1.5-pro");
        assert_eq!(p.input_cost_per_m, Decimal::from_str("1.25").unwrap());
    }

    // ── Cross-provider (Llama, DeepSeek) ─────────────────────

    #[test]
    fn test_llama_any_provider() {
        // Llama matches regardless of provider
        let p = get_model_pricing_fallback("together", "llama-3.1-70b-instruct");
        assert_eq!(p.input_cost_per_m, Decimal::from_str("0.88").unwrap());
    }

    #[test]
    fn test_deepseek_v3() {
        let p = get_model_pricing_fallback("deepseek", "deepseek-chat");
        assert_eq!(p.input_cost_per_m, Decimal::from_str("0.27").unwrap());
    }

    // ── Embeddings ───────────────────────────────────────────

    #[test]
    fn test_embeddings_zero_output_cost() {
        let p = get_model_pricing_fallback("openai", "text-embedding-3-small");
        assert_eq!(p.output_cost_per_m, Decimal::ZERO);
        assert!(p.input_cost_per_m > Decimal::ZERO);
    }

    // ── Unknown model → zero ─────────────────────────────────

    #[test]
    fn test_unknown_model_nonzero_fallback() {
        // 5A-2: Unknown models should get a non-zero fallback (not $0.00)
        let p = get_model_pricing_fallback("custom", "my-fine-tune");
        assert_eq!(p.input_cost_per_m, Decimal::from_str("1.00").unwrap());
        assert_eq!(p.output_cost_per_m, Decimal::from_str("3.00").unwrap());
    }

    // ── extract_usage ────────────────────────────────────────

    #[test]
    fn test_extract_usage_openai() {
        let body = r#"{"usage":{"prompt_tokens":100,"completion_tokens":50}}"#;
        let result = extract_usage("https://api.openai.com", body.as_bytes()).unwrap();
        assert_eq!(result, Some((100, 50)));
    }

    #[test]
    fn test_extract_usage_anthropic() {
        let body = r#"{"usage":{"input_tokens":200,"output_tokens":80}}"#;
        let result = extract_usage("https://api.anthropic.com", body.as_bytes()).unwrap();
        assert_eq!(result, Some((200, 80)));
    }

    #[test]
    fn test_extract_usage_gemini() {
        let body = r#"{"usageMetadata":{"promptTokenCount":300,"candidatesTokenCount":120}}"#;
        let result =
            extract_usage("https://generativelanguage.googleapis.com", body.as_bytes()).unwrap();
        assert_eq!(result, Some((300, 120)));
    }

    #[test]
    fn test_extract_usage_no_usage() {
        let body = r#"{"choices":[{"message":{"content":"hello"}}]}"#;
        let result = extract_usage("https://api.openai.com", body.as_bytes()).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_usage_not_json() {
        let result = extract_usage("https://api.openai.com", b"not json").unwrap();
        assert_eq!(result, None);
    }
}
