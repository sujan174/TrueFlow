"""
TrueFlow Python SDK — Official client for the TrueFlow Gateway.

Usage:

    # Agent proxy client
    from trueflow import TrueFlowClient
    client = TrueFlowClient(api_key="tf_v1_...")
    oai = client.openai()

    # Admin management client
    admin = TrueFlowClient.admin(admin_key="...")
    tokens = admin.tokens.list()
"""

from .client import TrueFlowClient, AsyncClient, HealthPoller, AsyncHealthPoller
from .resources.guardrails import (
    PRESET_PROMPT_INJECTION,
    PRESET_CODE_INJECTION,
    PRESET_PII_REDACTION,
    PRESET_PII_ENTERPRISE,
    PRESET_PII_BLOCK,
    PRESET_HIPAA,
    PRESET_PCI,
    PRESET_TOPIC_FENCE,
    PRESET_LENGTH_LIMIT,
)
from .decorators import with_guardrails, GuardrailContext
from .types import (
    Token,
    TokenCreateResponse,
    Credential,
    CredentialCreateResponse,
    Service,
    Policy,
    PolicyCreateResponse,
    AuditLog,
    ApprovalRequest,
    ApprovalDecision,
    RequestSummary,
    Response,
)
from .exceptions import (
    TrueFlowError,
    AuthenticationError,
    NotFoundError,
    RateLimitError,
    ValidationError,
    GatewayError,
    SpendCapError,
    AccessDeniedError,
    PermissionError,
    PolicyDeniedError,
    PayloadTooLargeError,
    ContentBlockedError,
)

# Backward-compatible alias
Client = TrueFlowClient

__version__ = "0.1.0"

__all__ = [
    # Clients
    "TrueFlowClient",
    "AsyncClient",
    "Client",
    # Health monitoring
    "HealthPoller",
    "AsyncHealthPoller",
    # Types
    "Token",
    "TokenCreateResponse",
    "Credential",
    "CredentialCreateResponse",
    "Service",
    "Policy",
    "PolicyCreateResponse",
    "AuditLog",
    "ApprovalRequest",
    "ApprovalDecision",
    "RequestSummary",
    "Response",
    # Exceptions
    "TrueFlowError",
    "AuthenticationError",
    "NotFoundError",
    "RateLimitError",
    "ValidationError",
    "GatewayError",
    "SpendCapError",
    "AccessDeniedError",
    "PermissionError",  # backward-compatible alias for AccessDeniedError
    "PolicyDeniedError",
    "PayloadTooLargeError",
    "ContentBlockedError",
    # Metadata
    "__version__",
    # Guardrail preset constants
    "PRESET_PROMPT_INJECTION",
    "PRESET_CODE_INJECTION",
    "PRESET_PII_REDACTION",
    "PRESET_PII_ENTERPRISE",
    "PRESET_PII_BLOCK",
    "PRESET_HIPAA",
    "PRESET_PCI",
    "PRESET_TOPIC_FENCE",
    "PRESET_LENGTH_LIMIT",
    # Decorators
    "with_guardrails",
    "GuardrailContext",
]
