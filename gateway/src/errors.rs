use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use thiserror::Error;

/// Canonical error body emitted by every TrueFlow endpoint.
///
/// Shape:
/// ```json
/// {
///   "error": {
///     "code":       "spend_cap_reached",
///     "message":    "Daily spend cap of $50.00 reached (USD)",
///     "request_id": "req_01J9...",
///     "type":       "billing_error",
///     "details":    { ... }
///   }
/// }
/// ```
#[derive(Debug, Error)]
pub enum AppError {
    #[error("token not found")]
    TokenNotFound,

    #[error("token revoked")]
    TokenRevoked,

    #[error("credential missing")]
    CredentialMissing,

    #[error("policy denied: {reason}")]
    PolicyDenied { policy: String, reason: String },

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("approval timeout")]
    ApprovalTimeout,

    #[error("approval rejected")]
    ApprovalRejected,

    #[error("rate limit exceeded")]
    RateLimitExceeded { retry_after_secs: u64 },

    #[error("spend cap reached: {message}")]
    SpendCapReached { message: String },

    #[error("payload too large")]
    PayloadTooLarge,

    #[error("content blocked: {reason}")]
    ContentBlocked {
        reason: String,
        details: Option<Value>,
    },

    #[error("all upstreams exhausted")]
    AllUpstreamsExhausted { details: Option<Value> },

    #[error("invalid config: {message}")]
    InvalidConfig { message: String },

    #[error("validation error: {message}")]
    ValidationError { message: String },

    /// Token creation/validation errors
    #[error("provider required for BYOK token")]
    ProviderRequired { message: String },

    #[error("custom URL required for provider: {provider}")]
    CustomUrlRequired { provider: String, message: String },

    #[error("upstream provider mismatch")]
    UpstreamProviderMismatch { upstream_provider: String, allowed_providers: Vec<String> },

    #[error("not found: {0}")]
    NotFound(String),

    #[error("upstream error: {0}")]
    Upstream(String),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        self.into_response_with_id(None)
    }
}

impl AppError {
    /// Emit an error response with a specific request ID attached.
    ///
    /// Use this in handlers that already hold a `request_id`:
    /// ```rust,ignore
    /// return AppError::TokenNotFound.into_response_with_id(Some(&request_id.to_string()));
    /// ```
    pub fn into_response_with_id(self, request_id: Option<&str>) -> Response {
        let req_id = request_id
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("req_{}", uuid::Uuid::new_v4().simple()));

        let (status, error_type, code, msg, details) = match &self {
            AppError::TokenNotFound => (
                StatusCode::UNAUTHORIZED,
                "authentication_error",
                "token_not_found",
                "Invalid or missing API token. Ensure TRUEFLOW_API_KEY is set correctly.".to_string(),
                None,
            ),
            AppError::TokenRevoked => (
                StatusCode::UNAUTHORIZED,
                "authentication_error",
                "token_revoked",
                "This token has been revoked. Create a new one at your TrueFlow dashboard.".to_string(),
                None,
            ),
            AppError::CredentialMissing => (
                StatusCode::BAD_GATEWAY,
                "configuration_error",
                "credential_missing",
                "The credential linked to this token no longer exists. Re-attach a credential via the dashboard.".to_string(),
                None,
            ),
            AppError::PolicyDenied { policy, reason } => (
                StatusCode::FORBIDDEN,
                "permission_error",
                "policy_denied",
                format!("Request blocked by policy '{}': {}", policy, reason),
                None,
            ),
            AppError::Forbidden(reason) => (
                StatusCode::FORBIDDEN,
                "permission_error",
                "model_access_denied",
                reason.clone(),
                None,
            ),
            AppError::ApprovalTimeout => (
                StatusCode::REQUEST_TIMEOUT,
                "timeout_error",
                "approval_timeout",
                "Request timed out waiting for human approval.".to_string(),
                None,
            ),
            AppError::ApprovalRejected => (
                StatusCode::FORBIDDEN,
                "permission_error",
                "approval_rejected",
                "Request was rejected by a reviewer.".to_string(),
                None,
            ),
            AppError::RateLimitExceeded { .. } => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limit_error",
                "rate_limit_exceeded",
                "Rate limit exceeded. Retry after the number of seconds in the Retry-After header.".to_string(),
                None,
            ),
            AppError::SpendCapReached { message } => (
                StatusCode::PAYMENT_REQUIRED,
                "billing_error",
                "spend_cap_reached",
                message.clone(),
                None,
            ),
            AppError::PayloadTooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "invalid_request_error",
                "payload_too_large",
                "Request body exceeds the maximum allowed size.".to_string(),
                None,
            ),
            AppError::ContentBlocked { reason, details } => (
                StatusCode::FORBIDDEN,
                "content_policy_error",
                "content_blocked",
                format!("Request blocked by content filter: {}", reason),
                details.clone(),
            ),
            AppError::AllUpstreamsExhausted { details } => (
                StatusCode::SERVICE_UNAVAILABLE,
                "upstream_error",
                "all_upstreams_exhausted",
                "All upstream targets are currently unhealthy. See 'details' for cooldown information.".to_string(),
                details.clone(),
            ),
            AppError::InvalidConfig { message } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_request_error",
                "invalid_config",
                message.clone(),
                None,
            ),
            AppError::ValidationError { message } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_request_error",
                "validation_error",
                message.clone(),
                None,
            ),
            AppError::ProviderRequired { message } => (
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                "provider_required",
                message.clone(),
                Some(json!({ "hint": "BYOK tokens require a 'provider' field when credential_id is not provided" })),
            ),
            AppError::CustomUrlRequired { provider, message } => (
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                "custom_url_required",
                message.clone(),
                Some(json!({
                    "provider": provider,
                    "providers_requiring_custom_url": ["azure", "bedrock", "custom"]
                })),
            ),
            AppError::UpstreamProviderMismatch { upstream_provider, allowed_providers } => (
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                "upstream_provider_mismatch",
                format!("Upstream provider '{}' is not in allowed_providers: {:?}", upstream_provider, allowed_providers),
                Some(json!({
                    "upstream_provider": upstream_provider,
                    "allowed_providers": allowed_providers
                })),
            ),
            AppError::NotFound(resource) => (
                StatusCode::NOT_FOUND,
                "invalid_request_error",
                "not_found",
                format!("{} not found", resource),
                None,
            ),
            AppError::Upstream(e) => {
                tracing::error!(error = %e, "Upstream error");
                (
                    StatusCode::BAD_GATEWAY,
                    "upstream_error",
                    "upstream_failed",
                    "The upstream service is unavailable. Please try again.".to_string(),
                    None,
                )
            }
            AppError::Database(e) => {
                tracing::error!(error = %e, "Database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "internal_server_error",
                    "An internal server error occurred. Please retry or contact support.".to_string(),
                    None,
                )
            }
            AppError::Redis(e) => {
                tracing::error!(error = %e, "Redis error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "internal_server_error",
                    "An internal server error occurred. Please retry or contact support.".to_string(),
                    None,
                )
            }
            AppError::Internal(e) => {
                tracing::error!(error = %e, "Internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "internal_server_error",
                    "An internal server error occurred. Please retry or contact support.".to_string(),
                    None,
                )
            }
        };

        let mut error_obj = json!({
            "code":       code,
            "message":    msg,
            "type":       error_type,
            "request_id": req_id,
        });

        if let Some(d) = details {
            error_obj["details"] = d;
        }

        let body = Json(json!({ "error": error_obj }));
        let mut response = (status, body).into_response();

        // Attach request ID as response header for easy debugging
        if let Ok(val) = axum::http::HeaderValue::from_str(&req_id) {
            response.headers_mut().insert("x-request-id", val);
        }

        // Retry-After and X-RateLimit-Reset headers for rate limit responses
        if let AppError::RateLimitExceeded { retry_after_secs } = &self {
            let retry_after = retry_after_secs.to_string();
            let reset_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() + retry_after_secs)
                .unwrap_or(*retry_after_secs);
            if let Ok(val) = axum::http::HeaderValue::from_str(&retry_after) {
                response.headers_mut().insert("retry-after", val);
            }
            if let Ok(val) = axum::http::HeaderValue::from_str(&reset_at.to_string()) {
                response.headers_mut().insert("x-ratelimit-reset", val);
            }
        }

        response
    }
}

/// Convenience: convert old-style `SpendCapReached` (no message) usages
impl From<&str> for AppError {
    fn from(msg: &str) -> Self {
        AppError::SpendCapReached {
            message: msg.to_string(),
        }
    }
}

impl From<AppError> for StatusCode {
    fn from(err: AppError) -> Self {
        match err {
            AppError::TokenNotFound | AppError::TokenRevoked => StatusCode::UNAUTHORIZED,
            AppError::CredentialMissing => StatusCode::BAD_GATEWAY,
            AppError::PolicyDenied { .. } | AppError::Forbidden(_) | AppError::ApprovalRejected => {
                StatusCode::FORBIDDEN
            }
            AppError::ApprovalTimeout => StatusCode::REQUEST_TIMEOUT,
            AppError::RateLimitExceeded { .. } => StatusCode::TOO_MANY_REQUESTS,
            AppError::SpendCapReached { .. } => StatusCode::PAYMENT_REQUIRED,
            AppError::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            AppError::ContentBlocked { .. } => StatusCode::FORBIDDEN,
            AppError::AllUpstreamsExhausted { .. } => StatusCode::SERVICE_UNAVAILABLE,
            AppError::InvalidConfig { .. } | AppError::ValidationError { .. } => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            AppError::ProviderRequired { .. }
            | AppError::CustomUrlRequired { .. }
            | AppError::UpstreamProviderMismatch { .. } => StatusCode::BAD_REQUEST,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Upstream(_) => StatusCode::BAD_GATEWAY,
            AppError::Database(_) | AppError::Redis(_) | AppError::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}
