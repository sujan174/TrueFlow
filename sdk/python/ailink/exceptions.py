"""
Custom exceptions for the TrueFlow SDK.

Provides clear, actionable error messages instead of raw httpx.HTTPStatusError.
Parses the canonical TrueFlow error format:

    {"error": {"code": "...", "message": "...", "type": "...", "request_id": "...", "details": {...}}}
"""

import httpx


class TrueFlowError(Exception):
    """Base exception for all TrueFlow SDK errors."""

    def __init__(
        self,
        message: str,
        status_code: int = None,
        response: httpx.Response = None,
        error_type: str = "",
        code: str = "",
        request_id: str = "",
    ):
        self.message = message
        self.status_code = status_code
        self.response = response
        self.error_type = error_type
        self.code = code
        self.request_id = request_id
        super().__init__(message)

    def __repr__(self) -> str:
        parts = [f"message={self.message!r}"]
        if self.status_code:
            parts.append(f"status_code={self.status_code}")
        if self.code:
            parts.append(f"code={self.code!r}")
        if self.request_id:
            parts.append(f"request_id={self.request_id!r}")
        return f"{self.__class__.__name__}({', '.join(parts)})"


class AuthenticationError(TrueFlowError):
    """Invalid or missing API key / admin key."""
    pass


class AccessDeniedError(TrueFlowError):
    """Valid credentials but insufficient permissions."""
    pass


# Backward-compatible alias — prefer AccessDeniedError in new code
# to avoid shadowing Python's built-in PermissionError.
PermissionError = AccessDeniedError


class NotFoundError(TrueFlowError):
    """Requested resource does not exist."""
    pass


class RateLimitError(TrueFlowError):
    """Rate limit exceeded. Check retry_after for backoff duration."""

    def __init__(self, message: str, retry_after: float = None, **kwargs):
        self.retry_after = retry_after
        super().__init__(message, **kwargs)


class ValidationError(TrueFlowError):
    """Request payload failed server-side validation."""
    pass


class PayloadTooLargeError(TrueFlowError):
    """Request body exceeds the gateway's 25 MB size limit."""
    pass


class SpendCapError(TrueFlowError):
    """Token spend cap has been reached."""
    pass


class PolicyDeniedError(AccessDeniedError):
    """Request was blocked by a gateway policy."""
    pass

class ContentBlockedError(AccessDeniedError):
    """Request was blocked by a content filter (jailbreak, harmful content, etc.)."""

    def __init__(self, message: str, matched_patterns: list = None, confidence: float = None, **kwargs):
        self.matched_patterns = matched_patterns or []
        self.confidence = confidence
        super().__init__(message, **kwargs)


class GatewayError(TrueFlowError):
    """Gateway returned a 5xx error."""
    pass


def _parse_error_body(response: httpx.Response) -> tuple:
    """
    Parse the gateway error response body.

    Returns (message, error_type, code, request_id, details).
    """
    try:
        body = response.json()
        error = body.get("error", {})
        if isinstance(error, dict):
            return (
                error.get("message", response.text),
                error.get("type", ""),
                error.get("code", ""),
                error.get("request_id", ""),
                error.get("details"),
            )
        elif isinstance(error, str):
            return error, "", "", "", None
        return response.text, "", "", "", None
    except Exception:
        return response.text, "", "", "", None


def raise_for_status(response: httpx.Response) -> None:
    """
    Check response status and raise the appropriate TrueFlow exception.

    Use this instead of response.raise_for_status() for better error messages.
    The exception will include:
    - error_type: machine-readable category (e.g. "rate_limit_error")
    - code: specific error code (e.g. "rate_limit_exceeded")
    - request_id: from error body + X-Request-Id header
    - details: feature-specific metadata dict (e.g. matched_patterns for content filters)
    """
    if response.is_success:
        return

    status = response.status_code
    message, error_type, code, body_req_id, details = _parse_error_body(response)
    request_id = body_req_id or response.headers.get("x-request-id", "")

    kwargs = {
        "status_code": status,
        "response": response,
        "error_type": error_type,
        "code": code,
        "request_id": request_id,
    }

    if status == 401:
        raise AuthenticationError(f"Authentication failed: {message}", **kwargs)
    elif status == 402:
        raise SpendCapError(f"Spend cap reached: {message}", **kwargs)
    elif status == 403:
        if code == "policy_denied":
            raise PolicyDeniedError(f"Policy denied: {message}", **kwargs)
        if code == "content_blocked":
            matched = (details or {}).get("matched_patterns", [])
            confidence = (details or {}).get("confidence")
            raise ContentBlockedError(
                f"Content blocked: {message}",
                matched_patterns=matched,
                confidence=confidence,
                **kwargs,
            )
        raise AccessDeniedError(f"Permission denied: {message}", **kwargs)
    elif status == 404:
        raise NotFoundError(f"Resource not found: {message}", **kwargs)
    elif status == 413:
        raise PayloadTooLargeError(f"Payload too large: {message}", **kwargs)
    elif status == 422:
        raise ValidationError(f"Validation error: {message}", **kwargs)
    elif status == 429:
        retry_after_raw = response.headers.get("retry-after")
        raise RateLimitError(
            f"Rate limit exceeded: {message}",
            retry_after=float(retry_after_raw) if retry_after_raw else None,
            **kwargs,
        )
    elif 400 <= status < 500:
        raise TrueFlowError(f"Client error ({status}): {message}", **kwargs)
    elif status >= 500:
        raise GatewayError(f"Gateway error ({status}): {message}", **kwargs)
