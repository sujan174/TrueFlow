"""Pydantic models for TrueFlow API responses."""
from __future__ import annotations

import re
import warnings
from dataclasses import dataclass, field as dataclass_field
from typing import List, Optional, Dict, Any
from datetime import datetime
from pydantic import BaseModel, ConfigDict, Field


def validate_model_pattern(pattern: str, field_name: str = "allowed_models") -> List[str]:
    """Validate a model glob pattern.

    Returns a list of warnings (empty if valid).
    Raises ValueError if the pattern is invalid.

    Args:
        pattern: The glob pattern to validate
        field_name: Field name for error messages

    Valid patterns:
        - "gpt-4o" - exact match
        - "gpt-*" - prefix match
        - "*-preview" - suffix match
        - "gpt-*-mini" - contains match
        - "*" - match all
    """
    warnings_list: List[str] = []

    # Check for empty string
    if not pattern or pattern.strip() == "":
        raise ValueError(f"{field_name}: pattern cannot be empty or whitespace")

    # Check for invalid characters
    # Allow: alphanumeric, hyphens, underscores, dots, asterisks, question marks
    if not re.match(r'^[\w\-\.\*\?]+$', pattern):
        invalid_chars = set(re.findall(r'[^\w\-\.\*\?]', pattern))
        raise ValueError(
            f"{field_name}: pattern '{pattern}' contains invalid characters: {invalid_chars}. "
            f"Allowed: letters, numbers, hyphens (-), underscores (_), dots (.), asterisks (*), question marks (?)"
        )

    # Check for excessive wildcards (more than 3 consecutive asterisks)
    if '***' in pattern or '****' in pattern:
        warnings_list.append(
            f"{field_name}: pattern '{pattern}' has excessive consecutive wildcards (***+). "
            f"Use a single * to match any characters."
        )

    # Check for excessive total wildcards (more than 5 in pattern)
    if pattern.count('*') > 5:
        warnings_list.append(
            f"{field_name}: pattern '{pattern}' has {pattern.count('*')} wildcards. "
            f"Consider simplifying the pattern."
        )

    # Check for common typos
    if '=' in pattern:
        warnings_list.append(
            f"{field_name}: pattern '{pattern}' contains '=' which is likely a typo. "
            f"Did you mean '-'? (e.g., 'gpt-4*' not 'gpt=4*')"
        )

    # Check for patterns that look like regex (common mistake)
    regex_chars = ['^', '$', '+', '[', ']', '(', ')', '|', '\\']
    for char in regex_chars:
        if char in pattern:
            warnings_list.append(
                f"{field_name}: pattern '{pattern}' contains '{char}' which looks like regex syntax. "
                f"Use glob patterns instead: * (any chars), ? (single char)."
            )
            break

    return warnings_list


def _validate_patterns_list(patterns: Optional[List[str]], field_name: str) -> None:
    """Validate a list of model patterns, raising on errors, warning on issues."""
    if patterns is None:
        return

    for pattern in patterns:
        warns = validate_model_pattern(pattern, field_name)
        for w in warns:
            warnings.warn(w, UserWarning)


@dataclass
class Upstream:
    """A single upstream target with weight and priority for load balancing.

    Usage::

        from trueflow.types import Upstream

        admin.tokens.create(
            name="my-token",
            upstream_url="https://api.openai.com",
            upstreams=[
                Upstream(url="https://api.openai.com", weight=80),
                Upstream(url="https://api.backup.com", weight=20, priority=2),
            ],
        )

    Attributes:
        url: The upstream API endpoint URL.
        weight: Load balancing weight (default 100).
        priority: Failover priority (lower = higher priority, default 1).
        credential_id: Optional credential ID to use for this upstream.
        model: Optional model override. If set, replaces the request model
               when this upstream is selected. Enables safe cross-provider failover.
        allowed_models: Glob patterns for model filtering. Only route requests
                        with matching models to this upstream. Use ["*"] for all models.
    """

    url: str
    weight: int = 100
    priority: int = 1
    credential_id: Optional[str] = None
    model: Optional[str] = None
    allowed_models: Optional[List[str]] = None  # None = accept all models

    def __post_init__(self) -> None:
        """Validate model patterns after initialization."""
        # Validate allowed_models patterns
        _validate_patterns_list(self.allowed_models, "allowed_models")

        # Validate model override if specified
        if self.model is not None:
            validate_model_pattern(self.model, "model")

    def to_dict(self) -> Dict[str, Any]:
        """Serialize to the gateway's upstream JSON format."""
        d: Dict[str, Any] = {
            "url": self.url,
            "weight": self.weight,
            "priority": self.priority,
        }
        if self.credential_id is not None:
            d["credential_id"] = self.credential_id
        if self.model is not None:
            d["model"] = self.model
        if self.allowed_models is not None:
            d["allowed_models"] = self.allowed_models
        return d

    def __repr__(self) -> str:
        return f"Upstream(url={self.url!r}, weight={self.weight}, priority={self.priority})"


class Providers:
    """Predefined provider configurations for easy upstream setup.

    Usage::

        from trueflow.types import Upstream, Providers

        # Use predefined provider configurations
        token = client.tokens.create(
            name="multi-provider",
            upstreams=[
                Providers.OPENAI,
                Providers.ANTHROPIC,
            ],
        )

        # Customize a provider preset
        custom_openai = Upstream(
            url=Providers.OPENAI.url,
            weight=70,
            model="gpt-4o-mini",  # Override to cheaper model
            allowed_models=["gpt-*"],
        )
    """

    # OpenAI - GPT and O-series models
    OPENAI = Upstream(
        url="https://api.openai.com/v1",
        allowed_models=["gpt-*", "o1-*", "o3-*", "text-*", "tts-*", "dall-e-*", "whisper-*"],
    )

    # Anthropic - Claude models
    ANTHROPIC = Upstream(
        url="https://api.anthropic.com/v1",
        allowed_models=["claude-*"],
    )

    # Google Gemini
    GEMINI = Upstream(
        url="https://generativelanguage.googleapis.com/v1beta",
        allowed_models=["gemini-*"],
    )

    # Groq - Fast inference for open models
    GROQ = Upstream(
        url="https://api.groq.com/openai/v1",
        allowed_models=["*"],  # Groq hosts many models (llama, mixtral, gemma, etc.)
    )

    # Mistral AI
    MISTRAL = Upstream(
        url="https://api.mistral.ai/v1",
        allowed_models=["mistral-*", "mixtral-*", "codestral-*", "devstral-*"],
    )

    # Cohere
    COHERE = Upstream(
        url="https://api.cohere.ai/v1",
        allowed_models=["command-*", "embed-*", "rerank-*"],
    )

    # Together AI - Many open models
    TOGETHER = Upstream(
        url="https://api.together.xyz/v1",
        allowed_models=["*"],  # Together hosts many open models
    )

    # OpenRouter - Unified API for many models
    OPENROUTER = Upstream(
        url="https://openrouter.ai/api/v1",
        allowed_models=["*"],  # OpenRouter supports many providers
    )



class TrueFlowModel(BaseModel):
    """Base model with dict-access compatibility for backward compatibility."""
    model_config = ConfigDict(extra="allow", populate_by_name=True)

    def __getitem__(self, item):
        return getattr(self, item)

    def __contains__(self, item):
        return item in self.model_dump()


class Token(TrueFlowModel):
    """A virtual token that maps an agent to a credential and upstream endpoint."""
    id: str
    name: str
    credential_id: Optional[str] = None
    upstream_url: str
    project_id: Optional[str] = None
    policy_ids: List[str] = Field(default_factory=list)
    scopes: List[str] = Field(default_factory=list)
    is_active: bool
    created_at: Optional[datetime] = None

    def __repr__(self) -> str:
        return f"Token(id={self.id!r}, name={self.name!r}, active={self.is_active})"


class Credential(TrueFlowModel):
    """An encrypted credential (API key) for an upstream provider."""
    id: str
    name: str
    provider: str
    created_at: Optional[datetime] = None

    def __repr__(self) -> str:
        return f"Credential(id={self.id!r}, name={self.name!r}, provider={self.provider!r})"


class Service(TrueFlowModel):
    """A registered external service for the Action Gateway."""
    id: str
    name: str
    description: str = ""
    base_url: str
    service_type: str = "generic"
    credential_id: Optional[str] = None
    is_active: bool = True
    created_at: Optional[datetime] = None
    updated_at: Optional[datetime] = None

    def __repr__(self) -> str:
        return f"Service(id={self.id!r}, name={self.name!r}, type={self.service_type!r})"


class Policy(TrueFlowModel):
    """A security policy applied to token requests."""
    id: str
    name: str
    mode: str
    phase: str = "pre"
    rules: List[Dict[str, Any]]

    def __repr__(self) -> str:
        return f"Policy(id={self.id!r}, name={self.name!r}, mode={self.mode!r}, phase={self.phase!r})"


class AuditLog(TrueFlowModel):
    """A single audit log entry for a proxied request."""
    id: str
    created_at: datetime
    method: str
    path: str
    upstream_status: Optional[int] = None
    response_latency_ms: Optional[int] = None
    agent_name: Optional[str] = None
    policy_result: Optional[str] = None
    hitl_required: bool = False
    hitl_decision: Optional[str] = None
    hitl_latency_ms: Optional[int] = None
    fields_redacted: Optional[List[str]] = Field(default_factory=list)
    shadow_violations: Optional[List[str]] = Field(default_factory=list)
    # LLM Observability
    model: Optional[str] = None
    prompt_tokens: Optional[int] = None
    completion_tokens: Optional[int] = None
    finish_reason: Optional[str] = None
    is_streaming: Optional[bool] = None
    # Caching & Router
    cache_hit: Optional[bool] = None

    def __repr__(self) -> str:
        return f"AuditLog(id={self.id!r}, method={self.method!r}, path={self.path!r}, status={self.upstream_status})"


class RequestSummary(TrueFlowModel):
    """Summary of the original request, embedded in approval requests."""
    method: str
    path: str
    agent: Optional[str] = None
    upstream: Optional[str] = None

    def __repr__(self) -> str:
        return f"RequestSummary({self.method} {self.path})"


class ApprovalRequest(TrueFlowModel):
    """A HITL approval request pending admin review."""
    id: str
    token_id: str
    status: str  # pending, approved, rejected, expired, timeout
    request_summary: RequestSummary
    expires_at: Optional[datetime] = None
    updated: Optional[bool] = None

    def __repr__(self) -> str:
        return f"ApprovalRequest(id={self.id!r}, status={self.status!r})"


class ApprovalDecision(TrueFlowModel):
    """The result of an admin approval decision."""
    id: str
    status: str
    updated: bool

    def __repr__(self) -> str:
        return f"ApprovalDecision(id={self.id!r}, status={self.status!r})"


class Response(TrueFlowModel):
    """Generic API response wrapper."""
    message: Optional[str] = None
    data: Optional[Dict[str, Any]] = None


class TokenCreateResponse(TrueFlowModel):
    """Response from creating a new token."""
    token_id: Optional[str] = None   # the tf_v1_... key (only returned once)
    id: Optional[str] = None         # internal UUID
    name: Optional[str] = None
    upstream_url: Optional[str] = None
    credential_id: Optional[str] = None
    project_id: Optional[str] = None


class CredentialCreateResponse(TrueFlowModel):
    """Response from creating a credential."""
    id: Optional[str] = None
    name: Optional[str] = None
    provider: Optional[str] = None


class PolicyCreateResponse(TrueFlowModel):
    """Response from creating a policy."""
    id: Optional[str] = None
    name: Optional[str] = None
    mode: Optional[str] = None
    phase: Optional[str] = None

