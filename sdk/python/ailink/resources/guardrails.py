"""Guardrails management resource for the TrueFlow admin client.

Enables developers to configure safety, privacy, and compliance guardrails
on API tokens — without touching the gateway config directly.

Example::

    admin = TrueFlowClient.admin(admin_key="...")

    # See what guardrails are available
    presets = admin.guardrails.list_presets()
    for p in presets:
        print(p["name"], "-", p["description"])

    # Attach guardrails to a token
    admin.guardrails.enable(
        token_id="tok_abc123",
        presets=["prompt_injection", "pii_enterprise"],
    )

    # Topic-restricted agent (only allow finance topics)
    admin.guardrails.enable(
        token_id="tok_finance_agent",
        presets=["topic_fence", "pii_redaction"],
        topic_allowlist=["finance", "investment", "portfolio"],
    )

    # Remove all guardrails from a token
    admin.guardrails.disable(token_id="tok_abc123")
"""

from typing import Any, Dict, List, Optional


# ── Available preset names (for IDE autocompletion) ────────────
PRESET_PROMPT_INJECTION = "prompt_injection"
"""Block DAN jailbreaks, harmful content, and code injection (35+ patterns, risk threshold 0.3)."""

PRESET_CODE_INJECTION = "code_injection"
"""Block SQL injection, shell commands, Python exec/eval, JS eval, data exfiltration."""

PRESET_PII_REDACTION = "pii_redaction"
"""Silently redact 8 PII types (SSN, email, credit card, phone, API key, IBAN, DOB, IP)."""

PRESET_PII_ENTERPRISE = "pii_enterprise"
"""Enterprise-grade: redact all 12 PII types including passport, AWS key, driver's license, MRN."""

PRESET_PII_BLOCK = "pii_block"
"""Block (HTTP 400) requests containing PII — for strict no-PII policies."""

PRESET_HIPAA = "hipaa"
"""Healthcare: redact SSN, email, phone, date-of-birth, MRN."""

PRESET_PCI = "pci"
"""Payment Card Industry: redact credit card numbers and API keys."""

PRESET_TOPIC_FENCE = "topic_fence"
"""Restrict agents to specific topics. Requires topic_allowlist or topic_denylist."""

PRESET_LENGTH_LIMIT = "length_limit"
"""Block requests with content exceeding 50,000 characters."""


class GuardrailsResource:
    """Guardrails management resource — configure safety, privacy, and compliance per token.

    Access via ``admin.guardrails``::

        admin = TrueFlowClient.admin(admin_key="...")
        admin.guardrails.enable("tok_abc123", ["prompt_injection", "pii_enterprise"])
    """

    def __init__(self, client) -> None:
        self._client = client

    # ── Preset Catalogue ───────────────────────────────────────

    def list_presets(self) -> List[Dict[str, Any]]:
        """Return the available guardrail presets from the gateway.

        Each preset is a dict with keys:
        - ``name`` (str): Preset identifier, e.g. ``"prompt_injection"``.
        - ``description`` (str): Human-readable description.
        - ``category`` (str): ``"safety"``, ``"privacy"``, or ``"compliance"``.
        - ``patterns`` (list[str], optional): PII/regex patterns included.
        - ``required_fields`` (list[str], optional): Fields that must be supplied.

        Returns:
            List of preset dicts.

        Example::

            presets = admin.guardrails.list_presets()
            safety = [p for p in presets if p["category"] == "safety"]
        """
        from ..exceptions import raise_for_status
        resp = self._client._http.get("/api/v1/guardrails/presets")
        raise_for_status(resp)
        return resp.json().get("presets", [])

    # ── Status ─────────────────────────────────────────────────

    def status(self, token_id: str) -> Dict[str, Any]:
        """Check current guardrails state for a token.

        Returns a dict with keys:
        - ``token_id`` (str)
        - ``has_guardrails`` (bool)
        - ``source`` (str | None): Who set them — ``"sdk"``, ``"dashboard"``, or ``"header"``.
        - ``policy_id`` (str | None)
        - ``policy_name`` (str | None)
        - ``presets`` (list[str]): Detected active presets.

        Example::

            st = admin.guardrails.status("tok_abc123")
            if st["has_guardrails"] and st["source"] == "dashboard":
                print("Warning: guardrails were set via dashboard")
        """
        from ..exceptions import raise_for_status
        resp = self._client._http.get("/api/v1/guardrails/status", params={"token_id": token_id})
        raise_for_status(resp)
        return resp.json()

    # ── Enable ─────────────────────────────────────────────────

    def enable(
        self,
        token_id: str,
        presets: List[str],
        *,
        topic_allowlist: Optional[List[str]] = None,
        topic_denylist: Optional[List[str]] = None,
    ) -> Dict[str, Any]:
        """Attach one or more guardrail presets to a token.

        Args:
            token_id:         The token to protect.
            presets:          One or more preset names. Use the ``PRESET_*`` constants
                              in this module for IDE autocompletion, or pass strings
                              such as ``"prompt_injection"``.
            topic_allowlist:  Only allow prompts that mention at least one of these
                              topics. Required when ``"topic_fence"`` is in *presets*.
            topic_denylist:   Block prompts that mention any of these topics.
                              Required when ``"topic_fence"`` is in *presets*.

        Returns:
            Dict with keys:
            - ``success`` (bool)
            - ``applied_presets`` (list[str])
            - ``policy_id`` (str | None)
            - ``policy_name`` (str)
            - ``skipped`` (list[str])

        Raises:
            :class:`~trueflow.ValidationError`: if a preset name is invalid.
            :class:`~trueflow.AuthenticationError`: if the admin key is missing/wrong.

        Examples::

            # Standard safety + privacy
            admin.guardrails.enable(
                "tok_abc123",
                ["prompt_injection", "pii_enterprise"],
            )

            # HIPAA-compliant healthcare agent
            admin.guardrails.enable(
                "tok_health_bot",
                ["hipaa", "prompt_injection"],
            )

            # Topic-restricted customer support agent
            admin.guardrails.enable(
                "tok_support",
                ["topic_fence", "pii_redaction"],
                topic_allowlist=["billing", "shipping", "returns"],
                topic_denylist=["competitors", "politics"],
            )

            # Chain everything for an enterprise deployment
            admin.guardrails.enable(
                "tok_prod",
                [
                    PRESET_PROMPT_INJECTION,
                    PRESET_CODE_INJECTION,
                    PRESET_PII_ENTERPRISE,
                    PRESET_LENGTH_LIMIT,
                ],
            )
        """
        from ..exceptions import raise_for_status
        payload: Dict[str, Any] = {"token_id": token_id, "presets": presets, "source": "sdk"}
        if topic_allowlist:
            payload["topic_allowlist"] = topic_allowlist
        if topic_denylist:
            payload["topic_denylist"] = topic_denylist
        resp = self._client._http.post("/api/v1/guardrails/enable", json=payload)
        raise_for_status(resp)
        return resp.json()

    # ── Disable ────────────────────────────────────────────────

    def disable(
        self,
        token_id: str,
        *,
        policy_name_prefix: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Remove guardrail policies from a token.

        By default removes **all** auto-generated guardrail policies. Pass
        *policy_name_prefix* to remove only a specific subset.

        Args:
            token_id:            The token to modify.
            policy_name_prefix:  If given, only remove policies whose name starts
                                 with this string (e.g. ``"guardrail:hipaa"``).

        Returns:
            Dict with keys ``success`` (bool) and ``removed`` (int).

        Example::

            # Remove everything
            admin.guardrails.disable("tok_abc123")

            # Remove only a specific policy set
            admin.guardrails.disable("tok_abc123", policy_name_prefix="guardrail:hipaa")
        """
        from ..exceptions import raise_for_status
        payload: Dict[str, Any] = {"token_id": token_id}
        if policy_name_prefix:
            payload["policy_name_prefix"] = policy_name_prefix
        resp = self._client._http.request(
            "DELETE", "/api/v1/guardrails/disable", json=payload
        )
        raise_for_status(resp)
        return resp.json()


class AsyncGuardrailsResource:
    """Async guardrails management resource.

    Access via ``async_admin.guardrails``::

        admin = AsyncClient.admin(admin_key="...")
        await admin.guardrails.enable("tok_abc123", ["prompt_injection"])
    """

    def __init__(self, client) -> None:
        self._client = client

    async def list_presets(self) -> List[Dict[str, Any]]:
        """Return the available guardrail presets from the gateway (async).

        See :meth:`GuardrailsResource.list_presets` for full documentation.
        """
        from ..exceptions import raise_for_status
        resp = await self._client._http.get("/api/v1/guardrails/presets")
        raise_for_status(resp)
        return resp.json().get("presets", [])

    async def status(self, token_id: str) -> Dict[str, Any]:
        """Check current guardrails state for a token (async).

        See :meth:`GuardrailsResource.status` for full documentation.
        """
        from ..exceptions import raise_for_status
        resp = await self._client._http.get("/api/v1/guardrails/status", params={"token_id": token_id})
        raise_for_status(resp)
        return resp.json()

    async def enable(
        self,
        token_id: str,
        presets: List[str],
        *,
        topic_allowlist: Optional[List[str]] = None,
        topic_denylist: Optional[List[str]] = None,
    ) -> Dict[str, Any]:
        """Attach guardrail presets to a token (async).

        See :meth:`GuardrailsResource.enable` for full documentation.

        Example::

            await admin.guardrails.enable(
                "tok_abc123",
                ["prompt_injection", "pii_enterprise"],
            )
        """
        from ..exceptions import raise_for_status
        payload: Dict[str, Any] = {"token_id": token_id, "presets": presets, "source": "sdk"}
        if topic_allowlist:
            payload["topic_allowlist"] = topic_allowlist
        if topic_denylist:
            payload["topic_denylist"] = topic_denylist
        resp = await self._client._http.post("/api/v1/guardrails/enable", json=payload)
        raise_for_status(resp)
        return resp.json()

    async def disable(
        self,
        token_id: str,
        *,
        policy_name_prefix: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Remove guardrail policies from a token (async).

        See :meth:`GuardrailsResource.disable` for full documentation.
        """
        from ..exceptions import raise_for_status
        payload: Dict[str, Any] = {"token_id": token_id}
        if policy_name_prefix:
            payload["policy_name_prefix"] = policy_name_prefix
        resp = await self._client._http.request(
            "DELETE", "/api/v1/guardrails/disable", json=payload
        )
        raise_for_status(resp)
        return resp.json()
