//! Microsoft Presidio adapter for NLP-based PII detection.
//!
//! Calls the Presidio Analyzer API (`POST /analyze`) to detect named entities
//! such as person names, locations, and medical terms that regex cannot catch.
//! Runs as an optional Docker sidecar — if unreachable, callers should fall back
//! to regex-only redaction (fail-open).

#![allow(dead_code)]

use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{PiiDetector, PiiEntity, PiiError};
use crate::utils::is_safe_webhook_url;

/// Presidio Analyzer HTTP client.
pub struct PresidioDetector {
    endpoint: String,
    client: Client,
    default_language: String,
    score_threshold: f32,
    timeout: Duration,
}

/// Request body for `POST /analyze`.
#[derive(Serialize)]
struct AnalyzeRequest<'a> {
    text: &'a str,
    language: &'a str,
    score_threshold: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    entities: Option<&'a [String]>,
}

/// A single entity from the Presidio Analyzer response.
#[derive(Deserialize)]
struct AnalyzerEntity {
    entity_type: String,
    start: usize,
    end: usize,
    score: f32,
}

impl PresidioDetector {
    /// Create a new Presidio detector.
    ///
    /// - `endpoint`: Base URL of the Presidio Analyzer service (e.g. `http://presidio:5002`).
    /// - `language`: Default language code (e.g. `"en"`).
    /// - `score_threshold`: Minimum confidence score (0.0–1.0).
    /// - `timeout`: Request timeout.
    pub fn new(
        endpoint: String,
        language: String,
        score_threshold: f32,
        timeout: Duration,
    ) -> Self {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();

        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            client,
            default_language: language,
            score_threshold,
            timeout,
        }
    }

    /// Build from an `NlpBackendConfig`.
    pub fn from_config(
        config: &crate::models::policy::NlpBackendConfig,
        timeout: Duration,
    ) -> Self {
        Self::new(
            config.endpoint.clone(),
            config.language.clone(),
            config.score_threshold,
            timeout,
        )
    }
}

#[async_trait::async_trait]
impl PiiDetector for PresidioDetector {
    async fn detect(
        &self,
        text: &str,
        language: Option<&str>,
    ) -> Result<Vec<PiiEntity>, PiiError> {
        let lang = language.unwrap_or(&self.default_language);
        let url = format!("{}/analyze", self.endpoint);

        // SEC: SSRF protection - validate endpoint before making HTTP request
        if !is_safe_webhook_url(&url).await {
            return Err(PiiError::Unavailable(
                "Presidio endpoint blocked by SSRF protection".to_string()
            ));
        }

        let request = AnalyzeRequest {
            text,
            language: lang,
            score_threshold: self.score_threshold,
            entities: None,
        };

        let response = tokio::time::timeout(self.timeout, async {
            self.client
                .post(&url)
                .json(&request)
                .send()
                .await
                .map_err(|e| PiiError::Unavailable(e.to_string()))
        })
        .await
        .map_err(|_| PiiError::Timeout(self.timeout.as_secs()))??;

        if !response.status().is_success() {
            return Err(PiiError::Unavailable(format!(
                "Presidio returned status {}",
                response.status()
            )));
        }

        let entities: Vec<AnalyzerEntity> = response
            .json()
            .await
            .map_err(|e| PiiError::Parse(e.to_string()))?;

        Ok(entities
            .into_iter()
            .filter_map(|e| {
                // Convert character offsets to byte offsets for correct UTF-8 slicing.
                // Presidio returns character offsets, but Rust strings use byte indices.
                let byte_start = text.char_indices()
                    .nth(e.start)
                    .map(|(i, _)| i)?;
                let byte_end = text.char_indices()
                    .nth(e.end)
                    .map(|(i, _)| i)?;

                let entity_text = text.get(byte_start..byte_end)?;
                Some(PiiEntity {
                    entity_type: e.entity_type,
                    start: byte_start,
                    end: byte_end,
                    score: e.score,
                    text: entity_text.to_string(),
                })
            })
            .collect())
    }

    fn name(&self) -> &str {
        "presidio"
    }
}

/// Detect entities using the Presidio adapter, with a specific set of entity
/// types. This is the entry point used by the proxy handler.
pub async fn detect_with_entities(
    detector: &PresidioDetector,
    text: &str,
    language: Option<&str>,
    entities: &[String],
) -> Result<Vec<PiiEntity>, PiiError> {
    if entities.is_empty() {
        return detector.detect(text, language).await;
    }

    let lang = language.unwrap_or(&detector.default_language);
    let url = format!("{}/analyze", detector.endpoint);

    // SEC: SSRF protection - validate endpoint before making HTTP request
    if !is_safe_webhook_url(&url).await {
        return Err(PiiError::Unavailable(
            "Presidio endpoint blocked by SSRF protection".to_string()
        ));
    }

    let request = AnalyzeRequest {
        text,
        language: lang,
        score_threshold: detector.score_threshold,
        entities: Some(entities),
    };

    let response = tokio::time::timeout(detector.timeout, async {
        detector
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| PiiError::Unavailable(e.to_string()))
    })
    .await
    .map_err(|_| PiiError::Timeout(detector.timeout.as_secs()))??;

    if !response.status().is_success() {
        return Err(PiiError::Unavailable(format!(
            "Presidio returned status {}",
            response.status()
        )));
    }

    let raw_entities: Vec<AnalyzerEntity> = response
        .json()
        .await
        .map_err(|e| PiiError::Parse(e.to_string()))?;

    Ok(raw_entities
        .into_iter()
        .filter_map(|e| {
            // Convert character offsets to byte offsets for correct UTF-8 slicing.
            // Presidio returns character offsets, but Rust strings use byte indices.
            let byte_start = text.char_indices()
                .nth(e.start)
                .map(|(i, _)| i)?;
            let byte_end = text.char_indices()
                .nth(e.end)
                .map(|(i, _)| i)?;

            let entity_text = text.get(byte_start..byte_end)?;
            Some(PiiEntity {
                entity_type: e.entity_type,
                start: byte_start,
                end: byte_end,
                score: e.score,
                text: entity_text.to_string(),
            })
        })
        .collect())
}
