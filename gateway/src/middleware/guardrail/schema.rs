// ── JSON Schema Validation ────────────────────────────────────

/// Result of a JSON Schema validation check.
pub struct SchemaValidationResult {
    pub valid: bool,
    /// List of validation error messages (empty when `valid == true`).
    pub errors: Vec<String>,
    /// The parsed JSON value that was validated (may differ from raw response if
    /// we extracted JSON from a markdown code block).
    #[allow(dead_code)]
    pub validated_value: Option<serde_json::Value>,
}

/// Validate an LLM response body against a JSON Schema.
///
/// Portkey-compatible: extracts the first JSON block from markdown if the
/// raw content is wrapped in ` ```json ... ``` `.
///
/// Works on:
/// - Full OpenAI chat completion responses (extracts `choices[0].message.content`)
/// - Raw JSON objects / arrays (validated directly)
pub fn validate_schema(
    response_body: &serde_json::Value,
    schema: &serde_json::Value,
) -> SchemaValidationResult {
    // 1. Compile the schema
    let compiled = match jsonschema::JSONSchema::compile(schema) {
        Ok(c) => c,
        Err(e) => {
            return SchemaValidationResult {
                valid: false,
                errors: vec![format!("Invalid JSON Schema: {}", e)],
                validated_value: None,
            };
        }
    };

    // 2. Extract the candidate value to validate
    //    Priority: choices[0].message.content → full response body
    let candidate = extract_content_for_validation(response_body);

    // 3. Validate — eagerly collect errors so we don't need to keep `compiled` borrowed
    let errors: Vec<String> = match compiled.validate(&candidate) {
        Ok(()) => vec![],
        Err(errs) => errs
            .map(|e| format!("{} (at {})", e, e.instance_path))
            .collect(),
    };

    let valid = errors.is_empty();
    SchemaValidationResult {
        valid,
        errors,
        validated_value: Some(candidate),
    }
}

/// Extract the value to validate from an LLM response.
/// Tries `choices[0].message.content` first (OpenAI format), then uses
/// the full response body. If the content is a JSON-wrapped markdown block,
/// the inner JSON is parsed and returned.
fn extract_content_for_validation(body: &serde_json::Value) -> serde_json::Value {
    // Try to get the assistant's message content from an OpenAI-style response
    let content_str = body
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str());

    if let Some(raw) = content_str {
        // Try to parse as JSON directly
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) {
            return v;
        }
        // Try to extract JSON from a markdown code block: ```json ... ```
        if let Some(inner) = extract_json_from_markdown(raw) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&inner) {
                return v;
            }
        }
        // Fall back: treat as plain string value
        return serde_json::Value::String(raw.to_owned());
    }

    // No message content — validate the full response body
    body.clone()
}

/// Extract JSON from a markdown code block like:
/// ```json
/// { ... }
/// ```
fn extract_json_from_markdown(text: &str) -> Option<String> {
    // Find opening fence
    let start = text.find("```json").or_else(|| text.find("```JSON"))?;
    let after_fence = &text[start + 7..]; // skip "```json"
                                          // Find closing fence
    let end = after_fence.find("```")?;
    Some(after_fence[..end].trim().to_owned())
}

#[cfg(test)]
mod schema_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_valid_schema_passes() {
        let schema = json!({
            "type": "object",
            "required": ["answer"],
            "properties": {
                "answer": { "type": "string" }
            }
        });
        let response = json!({ "answer": "42" });
        let result = validate_schema(&response, &schema);
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_invalid_schema_fails() {
        let schema = json!({
            "type": "object",
            "required": ["answer", "confidence"],
            "properties": {
                "answer": { "type": "string" },
                "confidence": { "type": "number" }
            }
        });
        let response = json!({ "answer": "42" }); // missing confidence
        let result = validate_schema(&response, &schema);
        assert!(!result.valid);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_extracts_from_openai_response() {
        let schema = json!({
            "type": "object",
            "required": ["score"],
            "properties": {
                "score": { "type": "number" }
            }
        });
        let response = json!({
            "choices": [{
                "message": {
                    "content": "{\"score\": 0.9}"
                }
            }]
        });
        let result = validate_schema(&response, &schema);
        assert!(result.valid);
    }

    #[test]
    fn test_extracts_from_markdown_code_block() {
        let schema = json!({
            "type": "object",
            "required": ["score"],
            "properties": {
                "score": { "type": "number" }
            }
        });
        let response = json!({
            "choices": [{
                "message": {
                    "content": "Here is the result:\n```json\n{\"score\": 0.9}\n```"
                }
            }]
        });
        let result = validate_schema(&response, &schema);
        assert!(result.valid);
    }

    #[test]
    fn test_invalid_schema_definition_returns_error() {
        // A schema with deliberately broken content
        let schema = json!({ "type": 12345 }); // type must be a string
        let response = json!({ "answer": "42" });
        let result = validate_schema(&response, &schema);
        assert!(!result.valid);
    }
}
