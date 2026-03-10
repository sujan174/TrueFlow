//! Config document schema (v2) — extends the original v1 with spend caps.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Top-level config document for plan/apply/export.
///
/// v2 adds `spend_caps` to tokens. The server-side import API still uses v1
/// for policies/tokens; spend caps are applied via separate API calls.
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigDoc {
    /// Schema version — "2" for this format.
    pub version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policies: Vec<PolicySpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tokens: Vec<TokenSpec>,
}

/// A policy specification in the config file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolicySpec {
    pub name: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_phase")]
    pub phase: String,
    pub rules: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<serde_json::Value>,
}

/// A token specification in the config file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenSpec {
    pub name: String,
    pub upstream_url: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policies: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_level: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub spend_caps: BTreeMap<String, f64>,
}

fn default_mode() -> String {
    "enforce".into()
}
fn default_phase() -> String {
    "request".into()
}

impl ConfigDoc {
    /// Load a config document from a YAML or JSON file.
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("yaml");
        let doc: Self = match ext {
            "json" => serde_json::from_str(&content)?,
            _ => serde_yaml::from_str(&content)?,
        };
        anyhow::ensure!(
            doc.version == "2",
            "unsupported config version '{}' (expected '2')",
            doc.version
        );
        Ok(doc)
    }

    /// Serialize to YAML.
    pub fn to_yaml(&self) -> anyhow::Result<String> {
        Ok(serde_yaml::to_string(self)?)
    }
}
