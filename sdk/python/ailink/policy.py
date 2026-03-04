"""Fluent typed policy builder DSL for TrueFlow.

Build structured policies without writing raw JSON::

    from trueflow.policy import Policy, when, deny, rate_limit, redact, content_filter

    policy = (
        Policy("block-gpt4")
        .add_rule(when(field="body.model", op="glob", value="gpt-4*"), then=deny())
        .shadow()
    )
    admin.policies.create(policy)

All classes serialize to the existing JSON policy format — no gateway changes needed.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Union


# ── Conditions ────────────────────────────────────────────────────────────────


@dataclass
class Condition:
    """A single field → operator → value condition."""

    field: str
    op: str = "eq"
    value: Any = None

    def to_dict(self) -> Dict[str, Any]:
        return {"field": self.field, "op": self.op, "value": self.value}


def when(field: str, op: str = "eq", value: Any = None) -> Condition:
    """Create a simple condition — ``when(field="body.model", op="eq", value="gpt-4o")``."""
    return Condition(field=field, op=op, value=value)


@dataclass
class AllOf:
    """AND combinator — all conditions must match."""

    conditions: List[Condition]

    def to_dict(self) -> Dict[str, Any]:
        return {"all_of": [c.to_dict() for c in self.conditions]}


@dataclass
class AnyOf:
    """OR combinator — at least one condition must match."""

    conditions: List[Condition]

    def to_dict(self) -> Dict[str, Any]:
        return {"any_of": [c.to_dict() for c in self.conditions]}


def all_of(*conditions: Condition) -> AllOf:
    """AND — all conditions must match."""
    return AllOf(list(conditions))


def any_of(*conditions: Condition) -> AnyOf:
    """OR — at least one condition must match."""
    return AnyOf(list(conditions))


ConditionLike = Union[Condition, AllOf, AnyOf]


# ── Actions ───────────────────────────────────────────────────────────────────


@dataclass
class DenyAction:
    message: str = "Request denied by policy"
    status: int = 403

    def to_dict(self) -> Dict[str, Any]:
        return {"type": "deny", "message": self.message, "status": self.status}


@dataclass
class RateLimitAction:
    window: str        # e.g. "1m", "1h"
    max_requests: int
    key: str = "per_token"  # per_token | per_agent | per_ip | per_user | global

    def to_dict(self) -> Dict[str, Any]:
        return {"type": "rate_limit", "window": self.window,
                "max_requests": self.max_requests, "key": self.key}


@dataclass
class RedactAction:
    patterns: List[str] = field(default_factory=list)  # e.g. ["email", "ssn", r"\d{16}"]
    direction: str = "both"                             # request | response | both
    fields: Optional[List[str]] = None                 # body fields to restrict to

    def to_dict(self) -> Dict[str, Any]:
        d: Dict[str, Any] = {"type": "redact", "patterns": self.patterns,
                              "direction": self.direction}
        if self.fields:
            d["fields"] = self.fields
        return d


@dataclass
class ContentFilterAction:
    block_jailbreak: bool = True
    block_harmful: bool = True
    topic_allowlist: List[str] = field(default_factory=list)
    topic_denylist: List[str] = field(default_factory=list)
    custom_patterns: List[str] = field(default_factory=list)
    whitelist_patterns: List[str] = field(default_factory=list)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "type": "content_filter",
            "block_jailbreak": self.block_jailbreak,
            "block_harmful": self.block_harmful,
            "topic_allowlist": self.topic_allowlist,
            "topic_denylist": self.topic_denylist,
            "custom_patterns": self.custom_patterns,
            "whitelist_patterns": self.whitelist_patterns,
        }


@dataclass
class RequireApprovalAction:
    timeout: str = "30m"   # e.g. "5m", "1h"
    approvers: List[str] = field(default_factory=list)
    message: str = ""

    def to_dict(self) -> Dict[str, Any]:
        return {"type": "require_approval", "timeout": self.timeout,
                "approvers": self.approvers, "message": self.message}


@dataclass
class WebhookAction:
    url: str
    events: List[str] = field(default_factory=list)
    timeout_ms: int = 5000

    def to_dict(self) -> Dict[str, Any]:
        return {"type": "webhook", "url": self.url, "events": self.events,
                "timeout_ms": self.timeout_ms}


ActionLike = Union[
    DenyAction, RateLimitAction, RedactAction,
    ContentFilterAction, RequireApprovalAction, WebhookAction,
]


# ── Convenience constructors ──────────────────────────────────────────────────


def deny(message: str = "Request denied by policy", status: int = 403) -> DenyAction:
    """Return a Deny action."""
    return DenyAction(message=message, status=status)


def rate_limit(window: str, max_requests: int, key: str = "per_token") -> RateLimitAction:
    """Return a RateLimit action. ``window`` uses shorthand like ``"1m"``, ``"1h"``."""
    return RateLimitAction(window=window, max_requests=max_requests, key=key)


def redact(
    patterns: Optional[List[str]] = None,
    direction: str = "both",
    fields: Optional[List[str]] = None,
) -> RedactAction:
    """Return a Redact action."""
    return RedactAction(patterns=patterns or [], direction=direction, fields=fields)


def content_filter(
    block_jailbreak: bool = True,
    block_harmful: bool = True,
    topic_denylist: Optional[List[str]] = None,
    topic_allowlist: Optional[List[str]] = None,
    custom_patterns: Optional[List[str]] = None,
    whitelist_patterns: Optional[List[str]] = None,
) -> ContentFilterAction:
    """Return a ContentFilter action."""
    return ContentFilterAction(
        block_jailbreak=block_jailbreak,
        block_harmful=block_harmful,
        topic_allowlist=topic_allowlist or [],
        topic_denylist=topic_denylist or [],
        custom_patterns=custom_patterns or [],
        whitelist_patterns=whitelist_patterns or [],
    )


def require_approval(
    timeout: str = "30m",
    approvers: Optional[List[str]] = None,
    message: str = "",
) -> RequireApprovalAction:
    """Return a RequireApproval action."""
    return RequireApprovalAction(timeout=timeout, approvers=approvers or [], message=message)


def webhook(url: str, events: Optional[List[str]] = None, timeout_ms: int = 5000) -> WebhookAction:
    """Return a Webhook action."""
    return WebhookAction(url=url, events=events or [], timeout_ms=timeout_ms)


# ── Rule & Policy ─────────────────────────────────────────────────────────────


@dataclass
class Rule:
    """A single condition→action rule."""

    condition: ConditionLike
    action: ActionLike

    def to_dict(self) -> Dict[str, Any]:
        return {
            "condition": self.condition.to_dict(),
            "action": self.action.to_dict(),
        }


class Policy:
    """Fluent typed builder for an TrueFlow policy.

    Usage::

        policy = (
            Policy("block-gpt4-and-redact-pii")
            .add_rule(when(field="body.model", op="glob", value="gpt-4*"), deny())
            .add_rule(when(field="always"), redact(patterns=["email", "ssn"]))
            .shadow()
        )
        admin.policies.create(policy)
    """

    def __init__(self, name: str, phase: str = "pre") -> None:
        self.name = name
        self.phase = phase
        self._rules: List[Rule] = []
        self._mode: str = "enforce"

    def add_rule(self, condition: ConditionLike, then: ActionLike) -> "Policy":
        """Add a condition → action rule and return self for chaining."""
        self._rules.append(Rule(condition=condition, action=then))
        return self

    def shadow(self) -> "Policy":
        """Set to shadow mode (log violations, don't block)."""
        self._mode = "shadow"
        return self

    def enforce(self) -> "Policy":
        """Set to enforce mode (block on violation, default)."""
        self._mode = "enforce"
        return self

    def to_dict(self) -> Dict[str, Any]:
        """Serialize to the gateway's JSON policy format."""
        return {
            "name": self.name,
            "mode": self._mode,
            "phase": self.phase,
            "rules": [r.to_dict() for r in self._rules],
        }

    def __repr__(self) -> str:
        return f"Policy(name={self.name!r}, mode={self._mode!r}, rules={len(self._rules)})"
