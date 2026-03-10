//! NLP-backed PII detection.
//!
//! Provides a generic `PiiDetector` trait and concrete backends (currently Presidio).
//! NLP detection augments the existing regex-based redaction in `redact.rs`,
//! catching unstructured PII like names, addresses, and multilingual entities
//! that regex cannot reliably match.

#![allow(dead_code)]

pub mod presidio;

use serde_json::Value;

// ── Trait & types ─────────────────────────────────────────────

/// A detected PII entity with span information.
#[derive(Debug, Clone)]
pub struct PiiEntity {
    /// Presidio entity type (e.g. "PERSON", "LOCATION", "PHONE_NUMBER").
    pub entity_type: String,
    /// Byte offset of the entity start in the original text.
    pub start: usize,
    /// Byte offset of the entity end in the original text.
    pub end: usize,
    /// Confidence score (0.0–1.0).
    pub score: f32,
    /// The matched text fragment.
    pub text: String,
}

/// Errors from NLP PII detection.
#[derive(Debug, thiserror::Error)]
pub enum PiiError {
    #[error("NLP backend unavailable: {0}")]
    Unavailable(String),
    #[error("NLP backend timed out after {0}s")]
    Timeout(u64),
    #[error("failed to parse NLP response: {0}")]
    Parse(String),
}

/// Trait for NLP-based PII detection backends.
#[async_trait::async_trait]
pub trait PiiDetector: Send + Sync {
    /// Detect PII entities in the given text.
    async fn detect(&self, text: &str, language: Option<&str>) -> Result<Vec<PiiEntity>, PiiError>;
    /// Human-readable backend name for logging.
    fn name(&self) -> &str;
}

// ── NLP entity application ───────────────────────────────────

/// Apply NLP-detected entities to a JSON body, replacing matched spans with
/// `[REDACTED_{TYPE}]` placeholders. Returns the set of entity types that
/// were redacted.
///
/// Entities are applied in reverse offset order to preserve positions.
pub fn apply_nlp_entities(body: &mut Value, entities: &[PiiEntity]) -> Vec<String> {
    let mut matched_types = Vec::new();
    apply_nlp_to_value(body, entities, &mut matched_types);
    matched_types
}

/// Recursively walk JSON and apply NLP entity redaction to string values.
fn apply_nlp_to_value(v: &mut Value, entities: &[PiiEntity], matched: &mut Vec<String>) {
    match v {
        Value::String(s) => {
            let new_val = redact_string_with_entities(s, entities, matched);
            if new_val != *s {
                *s = new_val;
            }
        }
        Value::Array(arr) => {
            for item in arr {
                apply_nlp_to_value(item, entities, matched);
            }
        }
        Value::Object(obj) => {
            for (_, val) in obj {
                apply_nlp_to_value(val, entities, matched);
            }
        }
        _ => {}
    }
}

/// Redact entity spans within a single string.
///
/// For each entity, checks if its `.text` appears in the string and replaces
/// occurrences with `[REDACTED_{TYPE}]`. This is position-independent so it
/// works even after regex has already modified the string.
fn redact_string_with_entities(
    s: &str,
    entities: &[PiiEntity],
    matched: &mut Vec<String>,
) -> String {
    let mut result = s.to_string();

    // Deduplicate entities by (text, entity_type) to avoid double-redacting
    let mut seen = std::collections::HashSet::new();

    for entity in entities {
        let key = (entity.text.as_str(), entity.entity_type.as_str());
        if !seen.insert(key) {
            continue;
        }

        let replacement = format!("[REDACTED_{}]", entity.entity_type.to_uppercase());

        if result.contains(&entity.text) {
            result = result.replace(&entity.text, &replacement);
            if !matched.contains(&entity.entity_type) {
                matched.push(entity.entity_type.clone());
            }
        }
    }

    result
}

/// Extract all string values from a JSON tree, concatenated with newlines.
/// Used to build the text payload for NLP analysis.
pub fn extract_text_from_value(v: &Value) -> String {
    let mut parts = Vec::new();
    collect_strings(v, &mut parts);
    parts.join("\n")
}

fn collect_strings(v: &Value, parts: &mut Vec<String>) {
    match v {
        Value::String(s) => {
            if !s.is_empty() {
                parts.push(s.clone());
            }
        }
        Value::Array(arr) => {
            for item in arr {
                collect_strings(item, parts);
            }
        }
        Value::Object(obj) => {
            for (_, val) in obj {
                collect_strings(val, parts);
            }
        }
        _ => {}
    }
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_apply_nlp_entities_simple() {
        let mut body = json!({
            "messages": [
                {"role": "user", "content": "My name is John Smith and I live in New York"}
            ]
        });

        let entities = vec![
            PiiEntity {
                entity_type: "PERSON".to_string(),
                start: 11,
                end: 21,
                score: 0.95,
                text: "John Smith".to_string(),
            },
            PiiEntity {
                entity_type: "LOCATION".to_string(),
                start: 36,
                end: 44,
                score: 0.90,
                text: "New York".to_string(),
            },
        ];

        let matched = apply_nlp_entities(&mut body, &entities);

        let content = body["messages"][0]["content"].as_str().unwrap();
        assert!(content.contains("[REDACTED_PERSON]"));
        assert!(content.contains("[REDACTED_LOCATION]"));
        assert!(!content.contains("John Smith"));
        assert!(!content.contains("New York"));
        assert!(matched.contains(&"PERSON".to_string()));
        assert!(matched.contains(&"LOCATION".to_string()));
    }

    #[test]
    fn test_apply_nlp_entities_no_match() {
        let mut body = json!({"text": "Hello world"});
        let entities = vec![PiiEntity {
            entity_type: "PERSON".to_string(),
            start: 0,
            end: 5,
            score: 0.9,
            text: "Alice".to_string(),
        }];

        let matched = apply_nlp_entities(&mut body, &entities);
        assert_eq!(body["text"], "Hello world");
        assert!(matched.is_empty());
    }

    #[test]
    fn test_apply_nlp_entities_duplicate_dedup() {
        let mut body = json!({
            "a": "Tell John Smith about John Smith",
        });

        let entities = vec![
            PiiEntity {
                entity_type: "PERSON".to_string(),
                start: 5,
                end: 15,
                score: 0.95,
                text: "John Smith".to_string(),
            },
            PiiEntity {
                entity_type: "PERSON".to_string(),
                start: 22,
                end: 32,
                score: 0.95,
                text: "John Smith".to_string(),
            },
        ];

        let matched = apply_nlp_entities(&mut body, &entities);
        let text = body["a"].as_str().unwrap();
        assert_eq!(text, "Tell [REDACTED_PERSON] about [REDACTED_PERSON]");
        assert_eq!(matched.len(), 1);
    }

    #[test]
    fn test_apply_nlp_entities_nested_json() {
        let mut body = json!({
            "messages": [
                {"role": "system", "content": "You are a helpful assistant"},
                {"role": "user", "content": "Contact Dr. Jane Doe at 123 Main Street"}
            ]
        });

        let entities = vec![
            PiiEntity {
                entity_type: "PERSON".to_string(),
                start: 12,
                end: 24,
                score: 0.92,
                text: "Dr. Jane Doe".to_string(),
            },
            PiiEntity {
                entity_type: "LOCATION".to_string(),
                start: 28,
                end: 43,
                score: 0.85,
                text: "123 Main Street".to_string(),
            },
        ];

        let matched = apply_nlp_entities(&mut body, &entities);

        // System message untouched
        assert_eq!(
            body["messages"][0]["content"],
            "You are a helpful assistant"
        );
        // User message redacted
        let content = body["messages"][1]["content"].as_str().unwrap();
        assert!(content.contains("[REDACTED_PERSON]"));
        assert!(content.contains("[REDACTED_LOCATION]"));
        assert_eq!(matched.len(), 2);
    }

    #[test]
    fn test_extract_text_from_value() {
        let body = json!({
            "messages": [
                {"role": "user", "content": "Hello world"},
                {"role": "assistant", "content": "Hi there"}
            ],
            "model": "gpt-4"
        });

        let text = extract_text_from_value(&body);
        assert!(text.contains("Hello world"));
        assert!(text.contains("Hi there"));
        assert!(text.contains("gpt-4"));
    }

    #[test]
    fn test_extract_text_empty() {
        let body = json!({"count": 42, "flag": true});
        let text = extract_text_from_value(&body);
        assert!(text.is_empty());
    }
}
