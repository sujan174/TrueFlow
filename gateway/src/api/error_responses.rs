use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// Structured API error response
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub violations: Option<Vec<serde_json::Value>>,
}

/// Scope violation details for policy validation
#[derive(Debug, Serialize)]
pub struct ScopeViolation {
    pub model: String,
    pub detected_provider: String,
    pub violation_type: ViolationType,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ViolationType {
    ProviderNotAllowed { allowed: Vec<String> },
    ModelNotAllowed { allowed_patterns: Vec<String> },
}

impl ApiError {
    /// Create a policy scope violation error
    pub fn policy_scope_violation(violations: Vec<ScopeViolation>) -> Self {
        Self {
            error: ErrorDetail {
                code: "policy_scope_violation".to_string(),
                message: "Policy routing targets exceed token's allowed scope".to_string(),
                violations: Some(
                    violations
                        .into_iter()
                        .map(|v| serde_json::to_value(v).unwrap_or_default())
                        .collect(),
                ),
            },
        }
    }

    /// Create a token not found error
    pub fn token_not_found() -> Self {
        Self {
            error: ErrorDetail {
                code: "token_not_found".to_string(),
                message: "The specified token was not found".to_string(),
                violations: None,
            },
        }
    }

    /// Create a generic validation error
    pub fn validation_error(message: impl Into<String>) -> Self {
        Self {
            error: ErrorDetail {
                code: "validation_error".to_string(),
                message: message.into(),
                violations: None,
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.error.code.as_str() {
            "token_not_found" => axum::http::StatusCode::NOT_FOUND,
            "policy_scope_violation" | "validation_error" => axum::http::StatusCode::BAD_REQUEST,
            _ => axum::http::StatusCode::BAD_REQUEST,
        };
        (status, axum::Json(self)).into_response()
    }
}
