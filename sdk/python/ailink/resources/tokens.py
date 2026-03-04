"""Resource for managing virtual tokens."""

from typing import List, Dict, Any, Optional, Iterator, AsyncIterator
from ..types import Token, TokenCreateResponse
from ..exceptions import raise_for_status


class TokensResource:
    """Management API resource for virtual tokens."""

    def __init__(self, client):
        self._client = client

    def list(self, project_id: Optional[str] = None) -> List[Token]:
        """List tokens, optionally filtered by project."""
        params = {}
        if project_id:
            params["project_id"] = project_id
        resp = self._client._http.get("/api/v1/tokens", params=params)
        raise_for_status(resp)
        return [Token(**item) for item in resp.json()]

    def list_all(self, project_id: Optional[str] = None, batch_size: int = 100) -> Iterator[Token]:
        """Auto-paginating iterator over all tokens."""
        offset = 0
        while True:
            params = {"limit": batch_size, "offset": offset}
            if project_id:
                params["project_id"] = project_id
            resp = self._client._http.get("/api/v1/tokens", params=params)
            raise_for_status(resp)
            batch = [Token(**item) for item in resp.json()]
            if not batch:
                break
            yield from batch
            if len(batch) < batch_size:
                break
            offset += batch_size

    def create(
        self,
        name: str,
        upstream_url: str,
        credential_id: Optional[str] = None,
        project_id: Optional[str] = None,
        policy_ids: Optional[List[str]] = None,
        circuit_breaker: Optional[Dict[str, Any]] = None,
        fallback_url: Optional[str] = None,
        upstreams: Optional[List[Any]] = None,  # List[Upstream | dict]
        log_level: Optional[str] = None,        # "metadata" | "redacted" | "full"
        expires_at: Optional[str] = None,       # ISO8601 timestamp string
    ) -> TokenCreateResponse:
        """
        Create a new virtual token.

        Args:
            name: Human-readable name for the token.
            upstream_url: The upstream API base URL to proxy to.
            credential_id: Optional vault credential ID. If omitted, operates
                in passthrough mode (agents provide their own API key via
                ``X-Real-Authorization``).
            project_id: Optional project scoping.
            policy_ids: Optional list of policy IDs to attach.
            circuit_breaker: Optional circuit breaker config dict.
                Examples:
                    ``{"enabled": False}`` — disable CB (useful for dev/test).
                    ``{"enabled": True, "failure_threshold": 5, "recovery_cooldown_secs": 60}``
            fallback_url: Convenience shorthand to set a single failover URL
                (creates two upstreams: primary at priority 1, fallback at priority 2).
            upstreams: Full upstream list as a mix of :class:`~trueflow.types.Upstream`
                objects and/or raw dicts. Takes precedence over ``fallback_url``.
            log_level: Log verbosity for this token — ``"metadata"``, ``"redacted"``,
                or ``"full"``. Defaults to the gateway global setting.

        Returns:
            A TokenCreateResponse containing the ``token_id`` and metadata.
        """
        payload: Dict[str, Any] = {
            "name": name,
            "upstream_url": upstream_url,
        }
        if credential_id:
            payload["credential_id"] = credential_id
        if project_id:
            payload["project_id"] = project_id
        if policy_ids:
            payload["policy_ids"] = policy_ids
        if circuit_breaker is not None:
            payload["circuit_breaker"] = circuit_breaker
        if fallback_url:
            payload["fallback_url"] = fallback_url
        if upstreams:
            payload["upstreams"] = [
                u.to_dict() if hasattr(u, "to_dict") else u for u in upstreams
            ]
        if log_level:
            payload["log_level_name"] = log_level
        if expires_at:
            payload["expires_at"] = expires_at
        resp = self._client._http.post("/api/v1/tokens", json=payload)
        raise_for_status(resp)
        return TokenCreateResponse(**resp.json())

    def revoke(self, token_id: str) -> Dict[str, Any]:
        """Revoke (soft-delete) a token."""
        resp = self._client._http.delete(f"/api/v1/tokens/{token_id}")
        raise_for_status(resp)
        # 204 No Content returns empty body
        if not resp.content:
            return {"revoked": True}
        return resp.json()

    def upstream_health(self) -> List[Dict[str, Any]]:
        """
        Get circuit breaker health status for all tracked upstream targets.

        Returns a list of upstream status objects with fields:
        - ``token_id``: the token this upstream belongs to
        - ``url``: upstream endpoint URL
        - ``is_healthy``: whether the circuit is closed
        - ``failure_count``: consecutive failures since last success
        - ``last_failure``: ISO timestamp of last failure (or null)
        """
        resp = self._client._http.get("/api/v1/health/upstreams")
        raise_for_status(resp)
        return resp.json()

    def get_circuit_breaker(self, token_id: str) -> Dict[str, Any]:
        """
        Get the circuit breaker configuration for a specific token.

        Returns a dict with keys: ``enabled``, ``failure_threshold``,
        ``recovery_cooldown_secs``, ``half_open_max_requests``.
        """
        resp = self._client._http.get(f"/api/v1/tokens/{token_id}/circuit-breaker")
        raise_for_status(resp)
        return resp.json()

    def set_circuit_breaker(
        self,
        token_id: str,
        *,
        enabled: bool = True,
        failure_threshold: int = 3,
        recovery_cooldown_secs: int = 30,
        half_open_max_requests: int = 1,
    ) -> Dict[str, Any]:
        """
        Update circuit breaker configuration for a token at runtime.

        Args:
            token_id: The token to configure.
            enabled: Toggle the circuit breaker on/off.
            failure_threshold: Consecutive failures before the circuit opens.
            recovery_cooldown_secs: Seconds before retrying an unhealthy upstream.
            half_open_max_requests: Requests allowed in half-open state.

        Returns:
            The updated config dict as stored on the gateway.

        Example::

            # Disable CB for a dev/test token
            admin.tokens.set_circuit_breaker(token_id, enabled=False)

            # Tune for a high-traffic token
            admin.tokens.set_circuit_breaker(
                token_id,
                enabled=True,
                failure_threshold=5,
                recovery_cooldown_secs=60,
            )
        """
        payload = {
            "enabled": enabled,
            "failure_threshold": failure_threshold,
            "recovery_cooldown_secs": recovery_cooldown_secs,
            "half_open_max_requests": half_open_max_requests,
        }
        resp = self._client._http.patch(
            f"/api/v1/tokens/{token_id}/circuit-breaker",
            json=payload,
        )
        raise_for_status(resp)
        return resp.json()

    def enable_guardrails(
        self,
        token_id: str,
        presets: List[str],
        topic_allowlist: Optional[List[str]] = None,
        topic_denylist: Optional[List[str]] = None,
    ) -> Dict[str, Any]:
        """
        Enable pre-configured guardrail presets for a token.
        
        Args:
            token_id: The token to configure.
            presets: List of preset names (e.g. 'pii_redaction', 'prompt_injection').
            topic_allowlist: Optional list for 'topic_fence' preset.
            topic_denylist: Optional list for 'topic_fence' preset.
        """
        payload: Dict[str, Any] = {
            "token_id": token_id,
            "presets": presets,
        }
        if topic_allowlist:
            payload["topic_allowlist"] = topic_allowlist
        if topic_denylist:
            payload["topic_denylist"] = topic_denylist
            
        resp = self._client._http.post("/api/v1/guardrails/enable", json=payload)
        raise_for_status(resp)
        return resp.json()

    def disable_guardrails(self, token_id: str, prefix: Optional[str] = None) -> Dict[str, Any]:
        """
        Disable auto-generated guardrails for a token.
        
        Args:
            token_id: The token to configure.
            prefix: Optional specific guardrail policy prefix to disable.
        """
        payload: Dict[str, Any] = {"token_id": token_id}
        if prefix:
            payload["policy_name_prefix"] = prefix
            
        resp = self._client._http.request("DELETE", "/api/v1/guardrails/disable", json=payload)
        raise_for_status(resp)
        return resp.json()


class AsyncTokensResource:
    """Async Management API resource for virtual tokens."""

    def __init__(self, client):
        self._client = client

    async def list(self, project_id: Optional[str] = None) -> List[Token]:
        """List tokens, optionally filtered by project."""
        params = {}
        if project_id:
            params["project_id"] = project_id
        resp = await self._client._http.get("/api/v1/tokens", params=params)
        raise_for_status(resp)
        return [Token(**item) for item in resp.json()]

    async def list_all(self, project_id: Optional[str] = None, batch_size: int = 100) -> AsyncIterator[Token]:
        """Auto-paginating async iterator over all tokens."""
        offset = 0
        while True:
            params = {"limit": batch_size, "offset": offset}
            if project_id:
                params["project_id"] = project_id
            resp = await self._client._http.get("/api/v1/tokens", params=params)
            raise_for_status(resp)
            batch = [Token(**item) for item in resp.json()]
            if not batch:
                break
            for item in batch:
                yield item
            if len(batch) < batch_size:
                break
            offset += batch_size

    async def create(
        self,
        name: str,
        upstream_url: str,
        credential_id: Optional[str] = None,
        project_id: Optional[str] = None,
        policy_ids: Optional[List[str]] = None,
        circuit_breaker: Optional[Dict[str, Any]] = None,
        fallback_url: Optional[str] = None,
        upstreams: Optional[List[Any]] = None,  # List[Upstream | dict]
        log_level: Optional[str] = None,        # "metadata" | "redacted" | "full"
        expires_at: Optional[str] = None,       # ISO8601 timestamp string
    ) -> TokenCreateResponse:
        """
        Create a new virtual token.

        Args:
            name: Human-readable name for the token.
            upstream_url: The upstream API base URL to proxy to.
            credential_id: Optional vault credential ID for managed credentials.
                If omitted, the token operates in passthrough mode.
            project_id: Optional project scoping.
            policy_ids: Optional list of policy IDs to attach.
            circuit_breaker: Optional circuit breaker config dict.
                Example: ``{"enabled": False}`` to disable CB for dev/test.
            fallback_url: Convenience shorthand to set a single failover URL
                (creates two upstreams: primary at priority 1, fallback at priority 2).
            upstreams: Full upstream list as a mix of :class:`~trueflow.types.Upstream`
                objects and/or raw dicts. Takes precedence over ``fallback_url``.
            log_level: Log verbosity for this token — ``"metadata"``, ``"redacted"``,
                or ``"full"``. Defaults to the gateway global setting.
            expires_at: Optional ISO8601 timestamp for token expiry.

        Returns:
            A TokenCreateResponse containing the 'token_id' and metadata.
        """
        payload: Dict[str, Any] = {
            "name": name,
            "upstream_url": upstream_url,
        }
        if credential_id:
            payload["credential_id"] = credential_id
        if project_id:
            payload["project_id"] = project_id
        if policy_ids:
            payload["policy_ids"] = policy_ids
        if circuit_breaker is not None:
            payload["circuit_breaker"] = circuit_breaker
        if fallback_url:
            payload["fallback_url"] = fallback_url
        if upstreams:
            payload["upstreams"] = [
                u.to_dict() if hasattr(u, "to_dict") else u for u in upstreams
            ]
        if log_level:
            payload["log_level_name"] = log_level
        if expires_at:
            payload["expires_at"] = expires_at
        resp = await self._client._http.post("/api/v1/tokens", json=payload)
        raise_for_status(resp)
        return TokenCreateResponse(**resp.json())

    async def revoke(self, token_id: str) -> Dict[str, Any]:
        """Revoke (soft-delete) a token."""
        resp = await self._client._http.delete(f"/api/v1/tokens/{token_id}")
        raise_for_status(resp)
        # 204 No Content returns empty body
        if not resp.content:
            return {"revoked": True}
        return resp.json()

    async def upstream_health(self) -> List[Dict[str, Any]]:
        """
        Get circuit breaker health status for all tracked upstream targets.

        Returns a list of upstream status objects with keys:
        ``token_id``, ``url``, ``is_healthy``, ``failure_count``, ``last_failure``.
        """
        resp = await self._client._http.get("/api/v1/health/upstreams")
        raise_for_status(resp)
        return resp.json()

    async def get_circuit_breaker(self, token_id: str) -> Dict[str, Any]:
        """
        Get the circuit breaker configuration for a specific token.

        Returns a dict with keys: ``enabled``, ``failure_threshold``,
        ``recovery_cooldown_secs``, ``half_open_max_requests``.
        """
        resp = await self._client._http.get(f"/api/v1/tokens/{token_id}/circuit-breaker")
        raise_for_status(resp)
        return resp.json()

    async def set_circuit_breaker(
        self,
        token_id: str,
        *,
        enabled: bool = True,
        failure_threshold: int = 3,
        recovery_cooldown_secs: int = 30,
        half_open_max_requests: int = 1,
    ) -> Dict[str, Any]:
        """
        Update circuit breaker configuration for a token at runtime.

        Args:
            token_id: The token to configure.
            enabled: Toggle the circuit breaker on/off.
            failure_threshold: Consecutive failures before the circuit opens.
            recovery_cooldown_secs: Seconds before retrying an unhealthy upstream.
            half_open_max_requests: Requests allowed in half-open state.
        """
        payload = {
            "enabled": enabled,
            "failure_threshold": failure_threshold,
            "recovery_cooldown_secs": recovery_cooldown_secs,
            "half_open_max_requests": half_open_max_requests,
        }
        resp = await self._client._http.patch(
            f"/api/v1/tokens/{token_id}/circuit-breaker",
            json=payload,
        )
        raise_for_status(resp)
        return resp.json()

    async def enable_guardrails(
        self,
        token_id: str,
        presets: List[str],
        topic_allowlist: Optional[List[str]] = None,
        topic_denylist: Optional[List[str]] = None,
    ) -> Dict[str, Any]:
        """
        Enable pre-configured guardrail presets for a token.
        
        Args:
            token_id: The token to configure.
            presets: List of preset names (e.g. 'pii_redaction', 'prompt_injection').
            topic_allowlist: Optional list for 'topic_fence' preset.
            topic_denylist: Optional list for 'topic_fence' preset.
        """
        payload: Dict[str, Any] = {
            "token_id": token_id,
            "presets": presets,
        }
        if topic_allowlist:
            payload["topic_allowlist"] = topic_allowlist
        if topic_denylist:
            payload["topic_denylist"] = topic_denylist
            
        resp = await self._client._http.post("/api/v1/guardrails/enable", json=payload)
        raise_for_status(resp)
        return resp.json()

    async def disable_guardrails(self, token_id: str, prefix: Optional[str] = None) -> Dict[str, Any]:
        """
        Disable auto-generated guardrails for a token.
        
        Args:
            token_id: The token to configure.
            prefix: Optional specific guardrail policy prefix to disable.
        """
        payload: Dict[str, Any] = {"token_id": token_id}
        if prefix:
            payload["policy_name_prefix"] = prefix
            
        resp = await self._client._http.request("DELETE", "/api/v1/guardrails/disable", json=payload)
        raise_for_status(resp)
        return resp.json()

