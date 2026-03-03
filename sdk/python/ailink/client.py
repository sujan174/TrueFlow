from __future__ import annotations
import uuid
import httpx
from contextlib import contextmanager, asynccontextmanager
from functools import cached_property
from typing import Optional, TYPE_CHECKING
import os
import time
from ._logging import log_request, log_response

if TYPE_CHECKING:
    import openai
    import anthropic
    from .resources.tokens import TokensResource, AsyncTokensResource
    from .resources.approvals import ApprovalsResource, AsyncApprovalsResource
    from .resources.audit import AuditResource, AsyncAuditResource
    from .resources.policies import PoliciesResource, AsyncPoliciesResource
    from .resources.credentials import CredentialsResource, AsyncCredentialsResource
    from .resources.projects import ProjectsResource, AsyncProjectsResource
    from .resources.services import ServicesResource, AsyncServicesResource


class AIlinkClient:
    """
    AIlink Gateway Client.

    For agent proxy operations (forwarding LLM requests through the gateway):

        client = AIlinkClient(api_key="ailink_v1_...")
        oai = client.openai()
        oai.chat.completions.create(...)

    For admin management operations:

        admin = AIlinkClient.admin(admin_key="...")
        admin.tokens.list()
    """

    def __init__(
        self,
        api_key: Optional[str] = None,
        gateway_url: Optional[str] = None,
        agent_name: Optional[str] = None,
        idempotency_key: Optional[str] = None,
        timeout: float = 30.0,
        max_retries: int = 2,
        *,
        _admin_key: Optional[str] = None,  # internal: set by admin() classmethod
        **kwargs,
    ):
        """
        Args:
            api_key: AIlink virtual token (starts with 'ailink_v1_'). Defaults to AILINK_API_KEY env var.
            gateway_url: URL of the AIlink gateway (default: http://localhost:8443 or AILINK_GATEWAY_URL env var)
            agent_name: Optional name for this agent (sent as X-AIlink-Agent-Name)
            idempotency_key: Optional key for idempotent requests
            timeout: Request timeout in seconds (default: 30)
            max_retries: Number of connection retries (default: 2)
            **kwargs: Additional arguments passed to httpx.Client
        """
        # Admin mode: authenticate with X-Admin-Key header
        if _admin_key:
            api_key = _admin_key
        else:
            api_key = api_key or os.environ.get("AILINK_API_KEY")
            if not api_key:
                from .exceptions import AIlinkError
                raise AIlinkError("No API key provided. Pass api_key= or set AILINK_API_KEY env var.")
        
        gateway_url = gateway_url or os.environ.get("AILINK_GATEWAY_URL", "http://localhost:8443")
        
        self.api_key = api_key
        self.gateway_url = gateway_url.rstrip("/")
        self._agent_name = agent_name

        if _admin_key:
            headers = {
                "X-Admin-Key": _admin_key,
                "Content-Type": "application/json",
            }
        else:
            headers = {"Authorization": f"Bearer {api_key}"}
            if agent_name:
                headers["X-AIlink-Agent-Name"] = agent_name
            if idempotency_key:
                headers["X-AIlink-Idempotency-Key"] = idempotency_key

        # Send SDK version on every request so the gateway can log it and
        # detect breaking incompatibilities (currently only logged, future: 426 upgrade hint)
        from . import __version__
        headers["X-AILink-SDK-Version"] = __version__

        # Only set retry transport if user hasn't provided their own transport
        if "transport" not in kwargs and max_retries > 0:
            kwargs["transport"] = httpx.HTTPTransport(retries=max_retries)
        
        _timings: dict = {}
        
        def _log_req(request: httpx.Request):
            _timings[id(request)] = time.perf_counter()
            log_request(request.method, str(request.url))
            
        def _log_res(response: httpx.Response):
            start = _timings.pop(id(response.request), time.perf_counter())
            elapsed = (time.perf_counter() - start) * 1000
            log_response(response.status_code, str(response.url), elapsed)
            
        self._http = httpx.Client(
            base_url=self.gateway_url,
            headers=headers,
            timeout=timeout,
            event_hooks={"request": [_log_req], "response": [_log_res]},
            **kwargs,
        )

    def __repr__(self) -> str:
        name = f", agent_name={self._agent_name!r}" if getattr(self, "_agent_name", None) else ""
        return f"AIlinkClient(gateway_url={self.gateway_url!r}{name})"

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        self.close()

    def close(self):
        """Close the underlying HTTP connection pool."""
        self._http.close()

    # ── Passthrough / BYOK ─────────────────────────────────────

    @contextmanager
    def with_upstream_key(self, key: str, header: str = "Bearer"):
        """
        Context manager for Passthrough (BYOK) mode.

        When the token has no stored credential, the gateway forwards
        whatever key you supply here directly to the upstream as the
        Authorization header.  The AIlink token still authenticates *you*
        to the gateway; this key authenticates the gateway to the upstream.

        Args:
            key:    The upstream API key (e.g. "sk-...").
            header: Auth scheme prefix (default: "Bearer").

        Example::

            with client.with_upstream_key("sk-my-openai-key") as byok:
                resp = byok.post("/v1/chat/completions", json={...})
        """
        auth_value = f"{header} {key}" if header else key
        scoped = _ScopedClient(
            self._http,
            extra_headers={"X-Real-Authorization": auth_value},
        )
        try:
            yield scoped
        finally:
            pass  # parent client owns the connection pool

    # ── Session Tracing ────────────────────────────────────────

    @contextmanager
    def trace(
        self,
        session_id: Optional[str] = None,
        parent_span_id: Optional[str] = None,
        properties: Optional[dict] = None,
    ):
        """
        Context manager that injects distributed-tracing headers.

        All requests made inside the block are tagged with the given
        session and span IDs, which appear in audit logs and can be
        used to correlate multi-step agent workflows.

        Args:
            session_id:     Logical session identifier (auto-generated if omitted).
            parent_span_id: Parent span for nested traces.
            properties:     Arbitrary JSON key-values attached to every audit log
                            for requests in this session. Stored as JSONB and
                            GIN-indexed for fast filtering.

        Example::

            with client.trace(
                session_id="agent-run-42",
                properties={"env": "prod", "customer": "acme", "feature": "research"}
            ) as t:
                t.post("/v1/chat/completions", json={...})  # step 1
                t.post("/v1/chat/completions", json={...})  # step 2

            # Then query the total cost of this agent run:
            # GET /api/v1/sessions/agent-run-42
        """
        import json
        sid = session_id or str(uuid.uuid4())
        extra: dict = {"x-session-id": sid}
        if parent_span_id:
            extra["x-parent-span-id"] = parent_span_id
        if properties:
            extra["x-properties"] = json.dumps(properties)
        scoped = _ScopedClient(self._http, extra_headers=extra)
        try:
            yield scoped
        finally:
            pass

    # ── Guardrails ─────────────────────────────────────────────

    @contextmanager
    def with_guardrails(self, presets: list[str]):
        """
        Context manager to apply guardrails on a per-request basis.

        Injects the ``X-AILink-Guardrails`` header with comma-separated
        preset names so the gateway applies them for this request only.

        Available presets: ``pii_redaction``, ``pii_block``, ``prompt_injection``.

        Args:
            presets: List of preset names.

        Example::

            with client.with_guardrails(["pii_redaction"]) as g:
                g.post("/v1/chat/completions", json={...})
        """
        if not presets:
            yield self
            return

        header_val = ",".join(presets)
        scoped = _ScopedClient(self._http, extra_headers={"X-AILink-Guardrails": header_val})
        try:
            yield scoped
        finally:
            pass

    # ── Health Check ───────────────────────────────────────────

    def is_healthy(self, timeout: float = 3.0) -> bool:
        """
        Returns True if the gateway is reachable and healthy, False otherwise.

        A fast, non-raising alternative to :meth:`health` — designed for
        conditional fallback logic::

            if client.is_healthy():
                oai = client.openai()   # use gateway
            else:
                oai = openai.OpenAI()   # bypass directly
        """
        try:
            resp = self._http.get("/healthz", timeout=timeout)
            return resp.status_code < 500
        except Exception:
            return False

    def health(self, timeout: float = 5.0) -> dict:
        """
        Check gateway health. Returns a status dict or raises GatewayError if unreachable.

        Returns::

            {"status": "ok", "gateway_url": "...", "http_status": 200}
        """
        from .exceptions import GatewayError
        try:
            resp = self._http.get("/healthz", timeout=timeout)
            return {"status": "ok", "gateway_url": self.gateway_url, "http_status": resp.status_code}
        except httpx.ConnectError:
            raise GatewayError(f"Gateway unreachable at {self.gateway_url}")
        except httpx.TimeoutException:
            raise GatewayError(f"Gateway health check timed out after {timeout}s")

    @contextmanager
    def with_fallback(self, fallback, *, health_timeout: float = 3.0):
        """
        Context manager for automatic gateway fallback.  Checks gateway health
        before entering the block and yields either a gateway-backed client or
        the provided fallback.

        Best practice — always supply a fallback so your agent keeps working
        even when the AIlink gateway is temporarily unreachable::

            import openai

            # Fallback: raw OpenAI client (no policy enforcement / audit)
            fallback_oai = openai.OpenAI(api_key=os.environ["OPENAI_API_KEY"])

            with client.with_fallback(fallback_oai) as oai:
                # `oai` is client.openai() if gateway is healthy,
                # or fallback_oai if gateway is down.
                response = oai.chat.completions.create(
                    model="gpt-4o",
                    messages=[{"role": "user", "content": "Hello"}],
                )

        The ``fallback`` can be any object — a raw provider SDK client,
        a lambda, a cached response dict, or ``None`` if you want to handle
        the down case yourself (``oai`` will be ``None``).

        Args:
            fallback:       Object to yield when the gateway is unhealthy.
            health_timeout: Seconds to wait for the health probe (default: 3).
        """
        if self.is_healthy(timeout=health_timeout):
            yield self.openai()
        else:
            import warnings
            warnings.warn(
                f"AIlink gateway at {self.gateway_url} is unreachable — "
                "using fallback client. Requests will bypass policy enforcement and audit logging.",
                stacklevel=3,
            )
            yield fallback

    # ── HTTP Methods ───────────────────────────────────────────

    def request(self, method: str, url: str, **kwargs) -> httpx.Response:
        """Send an HTTP request through the gateway."""
        return self._http.request(method, url, **kwargs)

    def get(self, url: str, **kwargs) -> httpx.Response:
        """Send a GET request."""
        return self._http.get(url, **kwargs)

    def post(self, url: str, **kwargs) -> httpx.Response:
        """Send a POST request."""
        return self._http.post(url, **kwargs)

    def put(self, url: str, **kwargs) -> httpx.Response:
        """Send a PUT request."""
        return self._http.put(url, **kwargs)

    def patch(self, url: str, **kwargs) -> httpx.Response:
        """Send a PATCH request."""
        return self._http.patch(url, **kwargs)

    def delete(self, url: str, **kwargs) -> httpx.Response:
        """Send a DELETE request."""
        return self._http.delete(url, **kwargs)

    # ── Admin Factory ──────────────────────────────────────────

    @classmethod
    def admin(cls, admin_key: Optional[str] = None, gateway_url: Optional[str] = None, **kwargs) -> "AIlinkClient":
        """
        Create an admin client for Management API operations.

        Args:
            admin_key: Admin key (X-Admin-Key header value). Defaults to AILINK_ADMIN_KEY.
            gateway_url: URL of the AIlink gateway
        """
        admin_key = admin_key or os.environ.get("AILINK_ADMIN_KEY")
        if not admin_key:
            from .exceptions import AIlinkError
            raise AIlinkError("No admin key provided. Pass admin_key= or set AILINK_ADMIN_KEY env var.")
            
        return cls(
            gateway_url=gateway_url,
            _admin_key=admin_key,
            **kwargs,
        )

    # ── Provider Factories ─────────────────────────────────────

    def openai(self) -> "openai.Client":
        """
        Returns a configured openai.Client that routes through the gateway.

        Requires 'openai' package: pip install ailink[openai]
        """
        try:
            import openai
        except ImportError:
            raise ImportError(
                "The 'openai' package is required for client.openai(). "
                "Install it with: pip install ailink[openai]\n"
                "Or standalone: pip install openai"
            ) from None

        return openai.Client(
            api_key=self.api_key,
            base_url=self.gateway_url,
            default_headers={"X-AIlink-Agent-Name": self._agent_name} if self._agent_name else None,
            max_retries=0,
        )

    def anthropic(self) -> "anthropic.Client":
        """
        Returns a configured anthropic.Client that routes through the gateway.

        Requires 'anthropic' package: pip install ailink[anthropic]
        """
        try:
            import anthropic
        except ImportError:
            raise ImportError(
                "The 'anthropic' package is required for client.anthropic(). "
                "Install it with: pip install ailink[anthropic]\n"
                "Or standalone: pip install anthropic"
            ) from None

        return anthropic.Client(
            api_key="AILINK_GATEWAY_MANAGED",
            base_url=self.gateway_url,
            default_headers={"Authorization": f"Bearer {self.api_key}"},
            max_retries=0,
        )

    # ── Resource Properties (cached) ───────────────────────────

    @cached_property
    def tokens(self) -> "TokensResource":
        from .resources.tokens import TokensResource
        return TokensResource(self)

    @cached_property
    def approvals(self) -> "ApprovalsResource":
        from .resources.approvals import ApprovalsResource
        return ApprovalsResource(self)

    @cached_property
    def audit(self) -> "AuditResource":
        from .resources.audit import AuditResource
        return AuditResource(self)

    @cached_property
    def policies(self) -> "PoliciesResource":
        from .resources.policies import PoliciesResource
        return PoliciesResource(self)

    @cached_property
    def credentials(self) -> "CredentialsResource":
        from .resources.credentials import CredentialsResource
        return CredentialsResource(self)

    @cached_property
    def projects(self) -> "ProjectsResource":
        from .resources.projects import ProjectsResource
        return ProjectsResource(self)

    @cached_property
    def services(self) -> "ServicesResource":
        from .resources.services import ServicesResource
        return ServicesResource(self)

    @cached_property
    def api_keys(self) -> "ApiKeysResource":
        from .resources.api_keys import ApiKeysResource
        return ApiKeysResource(self)

    @cached_property
    def webhooks(self):
        """Webhook subscription management — create, list, delete, test."""
        from .resources.webhooks import WebhooksResource
        return WebhooksResource(self)

    @cached_property
    def experiments(self):
        """Experiment tracking (A/B testing) — stub until P4.2 API ships."""
        from .resources.experiments import ExperimentsResource
        return ExperimentsResource(self)

    @cached_property
    def guardrails(self) -> "GuardrailsResource":
        """Guardrail management — enable, disable, and list safety/privacy presets per token."""
        from .resources.guardrails import GuardrailsResource
        return GuardrailsResource(self)

    @cached_property
    def billing(self) -> "BillingResource":
        """Billing and usage information."""
        from .resources.billing import BillingResource
        return BillingResource(self)

    @cached_property
    def analytics(self) -> "AnalyticsResource":
        from .resources.analytics import AnalyticsResource
        return AnalyticsResource(self)

    @cached_property
    def config(self) -> "ConfigResource":
        """Config-as-Code: export/import policies and tokens as YAML or JSON."""
        from .resources.config import ConfigResource
        return ConfigResource(self)

    @cached_property
    def realtime(self) -> "RealtimeResource":
        """Realtime WebSocket sessions — connect to OpenAI Realtime API via the gateway."""
        from .resources.realtime import RealtimeResource
        return RealtimeResource(self)

    @cached_property
    def batches(self) -> "BatchesResource":
        """Feature 10: Proxy OpenAI /v1/batches through the AILink gateway."""
        from .resources.batches import BatchesResource
        return BatchesResource(self)

    @cached_property
    def fine_tuning(self) -> "FineTuningResource":
        """Feature 10: Proxy OpenAI /v1/fine_tuning/jobs through the AILink gateway."""
        from .resources.fine_tuning import FineTuningResource
        return FineTuningResource(self)

    @cached_property
    def prompts(self) -> "PromptsResource":
        """Prompt management — CRUD, versioning, deployment, rendering."""
        from .resources.prompts import PromptsResource
        return PromptsResource(self)


class AsyncClient:
    """
    AIlink Gateway Async Client.

    Supports async context manager for clean resource management:

        async with AsyncClient(api_key="ailink_v1_...") as client:
            oai = client.openai()
    """

    def __init__(
        self,
        api_key: Optional[str] = None,
        gateway_url: Optional[str] = None,
        agent_name: Optional[str] = None,
        idempotency_key: Optional[str] = None,
        timeout: float = 30.0,
        max_retries: int = 2,
        **kwargs,
    ):
        """
        Args:
            api_key: AIlink virtual token. Defaults to AILINK_API_KEY env var.
            gateway_url: Gateway URL (default: http://localhost:8443 or AILINK_GATEWAY_URL)
            agent_name: Optional name for this agent
            idempotency_key: Optional key for idempotent requests
            timeout: Request timeout in seconds (default: 30)
            max_retries: Number of connection retries (default: 2)
            **kwargs: Arguments for httpx.AsyncClient
        """
        api_key = api_key or os.environ.get("AILINK_API_KEY")
        if not api_key:
            from .exceptions import AIlinkError
            raise AIlinkError("No API key provided. Pass api_key= or set AILINK_API_KEY env var.")
            
        gateway_url = gateway_url or os.environ.get("AILINK_GATEWAY_URL", "http://localhost:8443")
        
        self.api_key = api_key
        self.gateway_url = gateway_url.rstrip("/")
        self._agent_name = agent_name

        headers = {"Authorization": f"Bearer {api_key}"}
        if agent_name:
            headers["X-AIlink-Agent-Name"] = agent_name
        if idempotency_key:
            headers["X-AIlink-Idempotency-Key"] = idempotency_key

        # Only set retry transport if user hasn't provided their own transport
        if "transport" not in kwargs and max_retries > 0:
            kwargs["transport"] = httpx.AsyncHTTPTransport(retries=max_retries)
        
        _timings: dict = {}
        
        async def _alog_req(request: httpx.Request):
            _timings[id(request)] = time.perf_counter()
            log_request(request.method, str(request.url))
            
        async def _alog_res(response: httpx.Response):
            start = _timings.pop(id(response.request), time.perf_counter())
            elapsed = (time.perf_counter() - start) * 1000
            log_response(response.status_code, str(response.url), elapsed)
            
        self._http = httpx.AsyncClient(
            base_url=self.gateway_url,
            headers=headers,
            timeout=timeout,
            event_hooks={"request": [_alog_req], "response": [_alog_res]},
            **kwargs,
        )

    def __repr__(self) -> str:
        name = f", agent_name={self._agent_name!r}" if self._agent_name else ""
        return f"AsyncClient(gateway_url={self.gateway_url!r}{name})"

    async def __aenter__(self):
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self.close()

    async def close(self):
        """Close the underlying HTTP connection pool."""
        await self._http.aclose()

    # ── Passthrough / BYOK ─────────────────────────────────────

    @asynccontextmanager
    async def with_upstream_key(self, key: str, header: str = "Bearer"):
        """
        Async context manager for Passthrough (BYOK) mode.

        Example::

            async with client.with_upstream_key("sk-my-key") as byok:
                resp = await byok.post("/v1/chat/completions", json={...})
        """
        auth_value = f"{header} {key}" if header else key
        scoped = _AsyncScopedClient(
            self._http,
            extra_headers={"X-Real-Authorization": auth_value},
        )
        try:
            yield scoped
        finally:
            pass

    # ── Session Tracing ────────────────────────────────────────

    @asynccontextmanager
    async def trace(
        self,
        session_id: Optional[str] = None,
        parent_span_id: Optional[str] = None,
        properties: Optional[dict] = None,
    ):
        """
        Async context manager that injects distributed-tracing headers.

        Example::

            async with client.trace(
                session_id="conv-abc123",
                properties={"env": "prod", "customer": "acme"}
            ) as t:
                await t.post("/v1/chat/completions", json={...})
        """
        import json as _json
        sid = session_id or str(uuid.uuid4())
        extra: dict = {"x-session-id": sid}
        if parent_span_id:
            extra["x-parent-span-id"] = parent_span_id
        if properties:
            extra["x-properties"] = _json.dumps(properties)
        scoped = _AsyncScopedClient(self._http, extra_headers=extra)
        try:
            yield scoped
        finally:
            pass

    # ── Guardrails ─────────────────────────────────────────────

    @asynccontextmanager
    async def with_guardrails(self, presets: list[str]):
        """
        Async context manager to apply guardrails on a per-request basis.

        Injects the ``X-AILink-Guardrails`` header with comma-separated
        preset names so the gateway applies them for this request only.

        Available presets: ``pii_redaction``, ``pii_block``, ``prompt_injection``.

        Args:
            presets: List of preset names.

        Example::

            async with client.with_guardrails(["pii_redaction"]) as g:
                await g.post("/v1/chat/completions", json={...})
        """
        if not presets:
            yield self
            return

        header_val = ",".join(presets)
        scoped = _AsyncScopedClient(self._http, extra_headers={"X-AILink-Guardrails": header_val})
        try:
            yield scoped
        finally:
            pass

    # ── Health Check ───────────────────────────────────────────

    async def is_healthy(self, timeout: float = 3.0) -> bool:
        """
        Returns True if the gateway is reachable and healthy, False otherwise.
        Non-raising — safe to use in conditional fallback logic.
        """
        try:
            resp = await self._http.get("/healthz", timeout=timeout)
            return resp.status_code < 500
        except Exception:
            return False

    async def health(self, timeout: float = 5.0) -> dict:
        """
        Check gateway health. Returns a status dict or raises GatewayError if unreachable.
        """
        from .exceptions import GatewayError
        try:
            resp = await self._http.get("/healthz", timeout=timeout)
            return {"status": "ok", "gateway_url": self.gateway_url, "http_status": resp.status_code}
        except httpx.ConnectError:
            raise GatewayError(f"Gateway unreachable at {self.gateway_url}")
        except httpx.TimeoutException:
            raise GatewayError(f"Gateway health check timed out after {timeout}s")

    @asynccontextmanager
    async def with_fallback(self, fallback, *, health_timeout: float = 3.0):
        """
        Async context manager for automatic gateway fallback.
        Yields a gateway-backed async client, or ``fallback`` if the gateway is down::

            import openai
            fallback_oai = openai.AsyncOpenAI(api_key=os.environ["OPENAI_API_KEY"])

            async with client.with_fallback(fallback_oai) as oai:
                response = await oai.chat.completions.create(
                    model="gpt-4o",
                    messages=[{"role": "user", "content": "Hello"}],
                )

        Args:
            fallback:       Object to yield when the gateway is unhealthy.
            health_timeout: Seconds to wait for the health probe (default: 3).
        """
        if await self.is_healthy(timeout=health_timeout):
            yield self.openai()
        else:
            import warnings
            warnings.warn(
                f"AIlink gateway at {self.gateway_url} is unreachable — "
                "using fallback client. Requests will bypass policy enforcement and audit logging.",
                stacklevel=3,
            )
            yield fallback

    # ── HTTP Methods ───────────────────────────────────────────

    async def request(self, method: str, url: str, **kwargs) -> httpx.Response:
        """Send an HTTP request through the gateway."""
        return await self._http.request(method, url, **kwargs)

    async def get(self, url: str, **kwargs) -> httpx.Response:
        return await self._http.get(url, **kwargs)

    async def post(self, url: str, **kwargs) -> httpx.Response:
        return await self._http.post(url, **kwargs)

    async def put(self, url: str, **kwargs) -> httpx.Response:
        return await self._http.put(url, **kwargs)

    async def patch(self, url: str, **kwargs) -> httpx.Response:
        return await self._http.patch(url, **kwargs)

    async def delete(self, url: str, **kwargs) -> httpx.Response:
        return await self._http.delete(url, **kwargs)

    # ── Provider Factories ─────────────────────────────────────

    def openai(self) -> "openai.AsyncOpenAI":
        try:
            import openai
        except ImportError:
            raise ImportError("Please install 'openai' package: pip install ailink[openai]")

        return openai.AsyncOpenAI(
            api_key=self.api_key,
            base_url=self.gateway_url,
            default_headers={"X-AIlink-Agent-Name": self._agent_name} if self._agent_name else None,
            max_retries=0,
        )

    def anthropic(self) -> "anthropic.AsyncAnthropic":
        try:
            import anthropic
        except ImportError:
            raise ImportError("Please install 'anthropic' package: pip install ailink[anthropic]")

        return anthropic.AsyncAnthropic(
            api_key="AILINK_GATEWAY_MANAGED",
            base_url=self.gateway_url,
            default_headers={"Authorization": f"Bearer {self.api_key}"},
            max_retries=0,
        )

    # ── Resource Properties (cached) ───────────────────────────

    @cached_property
    def tokens(self) -> "AsyncTokensResource":
        from .resources.tokens import AsyncTokensResource
        return AsyncTokensResource(self)

    @cached_property
    def approvals(self) -> "AsyncApprovalsResource":
        from .resources.approvals import AsyncApprovalsResource
        return AsyncApprovalsResource(self)

    @cached_property
    def audit(self) -> "AsyncAuditResource":
        from .resources.audit import AsyncAuditResource
        return AsyncAuditResource(self)

    @cached_property
    def policies(self) -> "AsyncPoliciesResource":
        from .resources.policies import AsyncPoliciesResource
        return AsyncPoliciesResource(self)

    @cached_property
    def credentials(self) -> "AsyncCredentialsResource":
        from .resources.credentials import AsyncCredentialsResource
        return AsyncCredentialsResource(self)

    @cached_property
    def projects(self) -> "AsyncProjectsResource":
        from .resources.projects import AsyncProjectsResource
        return AsyncProjectsResource(self)

    @cached_property
    def services(self) -> "AsyncServicesResource":
        from .resources.services import AsyncServicesResource
        return AsyncServicesResource(self)

    @cached_property
    def api_keys(self) -> "AsyncApiKeysResource":
        from .resources.api_keys import AsyncApiKeysResource
        return AsyncApiKeysResource(self)

    @cached_property
    def billing(self) -> "AsyncBillingResource":
        from .resources.billing import AsyncBillingResource
        return AsyncBillingResource(self)

    @cached_property
    def analytics(self) -> "AsyncAnalyticsResource":
        from .resources.analytics import AsyncAnalyticsResource
        return AsyncAnalyticsResource(self)

    @cached_property
    def guardrails(self) -> "AsyncGuardrailsResource":
        """Guardrail management — enable, disable, and list safety/privacy presets per token."""
        from .resources.guardrails import AsyncGuardrailsResource
        return AsyncGuardrailsResource(self)

    @cached_property
    def config(self) -> "AsyncConfigResource":
        """Config-as-Code: export/import policies and tokens as YAML or JSON."""
        from .resources.config import AsyncConfigResource
        return AsyncConfigResource(self)

    @cached_property
    def realtime(self) -> "AsyncRealtimeResource":
        """Async Realtime WebSocket sessions via the gateway."""
        from .resources.realtime import AsyncRealtimeResource
        return AsyncRealtimeResource(self)

    @cached_property
    def batches(self) -> "AsyncBatchesResource":
        """Feature 10: Async proxy of OpenAI /v1/batches through the AILink gateway."""
        from .resources.batches import AsyncBatchesResource
        return AsyncBatchesResource(self)

    @cached_property
    def fine_tuning(self) -> "AsyncFineTuningResource":
        """Feature 10: Async proxy of OpenAI /v1/fine_tuning/jobs through the AILink gateway."""
        from .resources.fine_tuning import AsyncFineTuningResource
        return AsyncFineTuningResource(self)

    @cached_property
    def prompts(self) -> "AsyncPromptsResource":
        """Prompt management — CRUD, versioning, deployment, rendering."""
        from .resources.prompts import AsyncPromptsResource
        return AsyncPromptsResource(self)


# ── Scoped helpers (internal) ─────────────────────────────────────────────────
#
# These are lightweight wrappers returned by with_upstream_key() and trace().
# They share the parent's httpx client (connection pool) but merge extra headers
# into every request.  They intentionally expose only HTTP methods — no admin
# resources — because they are short-lived, single-purpose objects.


class _ScopedClient:
    """Sync scoped client that merges extra headers into every request."""

    def __init__(self, http: httpx.Client, extra_headers: dict):
        self._http = http
        self._extra = extra_headers

    def _merge(self, kwargs: dict) -> dict:
        existing = dict(kwargs.pop("headers", {}) or {})
        kwargs["headers"] = {**existing, **self._extra}
        return kwargs

    def request(self, method: str, url: str, **kwargs) -> httpx.Response:
        return self._http.request(method, url, **self._merge(kwargs))

    def get(self, url: str, **kwargs) -> httpx.Response:
        return self._http.get(url, **self._merge(kwargs))

    def post(self, url: str, **kwargs) -> httpx.Response:
        return self._http.post(url, **self._merge(kwargs))

    def put(self, url: str, **kwargs) -> httpx.Response:
        return self._http.put(url, **self._merge(kwargs))

    def patch(self, url: str, **kwargs) -> httpx.Response:
        return self._http.patch(url, **self._merge(kwargs))

    def delete(self, url: str, **kwargs) -> httpx.Response:
        return self._http.delete(url, **self._merge(kwargs))


class _AsyncScopedClient:
    """Async scoped client that merges extra headers into every request."""

    def __init__(self, http: httpx.AsyncClient, extra_headers: dict):
        self._http = http
        self._extra = extra_headers

    def _merge(self, kwargs: dict) -> dict:
        existing = dict(kwargs.pop("headers", {}) or {})
        kwargs["headers"] = {**existing, **self._extra}
        return kwargs

    async def request(self, method: str, url: str, **kwargs) -> httpx.Response:
        return await self._http.request(method, url, **self._merge(kwargs))

    async def get(self, url: str, **kwargs) -> httpx.Response:
        return await self._http.get(url, **self._merge(kwargs))

    async def post(self, url: str, **kwargs) -> httpx.Response:
        return await self._http.post(url, **self._merge(kwargs))

    async def put(self, url: str, **kwargs) -> httpx.Response:
        return await self._http.put(url, **self._merge(kwargs))

    async def patch(self, url: str, **kwargs) -> httpx.Response:
        return await self._http.patch(url, **self._merge(kwargs))

    async def delete(self, url: str, **kwargs) -> httpx.Response:
        return await self._http.delete(url, **self._merge(kwargs))


# ── Health Poller ─────────────────────────────────────────────────────────────
#
# Optional background health-monitoring helpers for agents that make many
# successive LLM calls and don't want to probe /healthz on every single request.
#
# Usage (sync):
#
#   poller = HealthPoller(client, interval=10)
#   poller.start()
#
#   # Hot path — zero extra HTTP requests:
#   if poller.is_healthy:
#       oai = client.openai()
#   else:
#       oai = fallback_client
#
#   poller.stop()
#
# Usage (async):
#
#   async with AsyncHealthPoller(client, interval=10) as poller:
#       if poller.is_healthy:
#           oai = client.openai()


class HealthPoller:
    """
    Background thread that continuously polls the AIlink gateway's ``/healthz``
    endpoint and caches the result, so agents can check health on the critical
    path without paying an HTTP round-trip per request.

    Args:
        client:   An ``AIlinkClient`` instance.
        interval: Seconds between health probes (default: 15).
        timeout:  Per-probe connect timeout in seconds (default: 3).

    Example::

        import openai
        from ailink import AIlinkClient, HealthPoller

        client = AIlinkClient(api_key=\"ailink_v1_...\")
        fallback = openai.OpenAI(api_key=os.environ[\"OPENAI_API_KEY\"])

        poller = HealthPoller(client, interval=10)
        poller.start()

        try:
            # Zero extra latency — uses cached health state
            oai = client.openai() if poller.is_healthy else fallback
            response = oai.chat.completions.create(
                model=\"gpt-4o\",
                messages=[{\"role\": \"user\", \"content\": \"Hello\"}],
            )
        finally:
            poller.stop()
    """

    def __init__(self, client: AIlinkClient, interval: float = 15.0, timeout: float = 3.0):
        self._client = client
        self._interval = interval
        self._timeout = timeout
        self._healthy: bool = True  # optimistic default
        self._thread: Optional[object] = None
        self._stop_event: Optional[object] = None

    @property
    def is_healthy(self) -> bool:
        """True if the last health probe succeeded."""
        return self._healthy

    def start(self) -> "HealthPoller":
        """Start the background polling thread. Returns self for chaining."""
        import threading
        self._stop_event = threading.Event()

        def _loop():
            while not self._stop_event.is_set():  # type: ignore[union-attr]
                self._healthy = self._client.is_healthy(timeout=self._timeout)
                self._stop_event.wait(self._interval)  # type: ignore[union-attr]

        self._thread = threading.Thread(target=_loop, daemon=True, name="ailink-health-poller")
        self._thread.start()  # type: ignore[union-attr]
        return self

    def stop(self):
        """Stop the background polling thread."""
        if self._stop_event:
            self._stop_event.set()  # type: ignore[union-attr]
        if self._thread:
            self._thread.join(timeout=2)  # type: ignore[union-attr]

    def __enter__(self) -> "HealthPoller":
        return self.start()

    def __exit__(self, *_):
        self.stop()


class AsyncHealthPoller:
    """
    Asyncio-native health poller for use in async agents.  Runs a background
    task that probes ``/healthz`` at a configurable interval.

    Use as an async context manager::

        import openai
        from ailink import AsyncClient, AsyncHealthPoller

        client = AsyncClient(api_key=\"ailink_v1_...\")
        fallback = openai.AsyncOpenAI(api_key=os.environ[\"OPENAI_API_KEY\"])

        async with AsyncHealthPoller(client, interval=10) as poller:
            oai = client.openai() if poller.is_healthy else fallback
            response = await oai.chat.completions.create(
                model=\"gpt-4o\",
                messages=[{\"role\": \"user\", \"content\": \"Hello\"}],
            )
    """

    def __init__(self, client: "AsyncClient", interval: float = 15.0, timeout: float = 3.0):
        self._client = client
        self._interval = interval
        self._timeout = timeout
        self._healthy: bool = True
        self._task: Optional[object] = None

    @property
    def is_healthy(self) -> bool:
        """True if the last health probe succeeded."""
        return self._healthy

    async def start(self) -> "AsyncHealthPoller":
        """Start the background polling asyncio task. Returns self for chaining."""
        import asyncio

        async def _loop():
            while True:
                self._healthy = await self._client.is_healthy(timeout=self._timeout)
                await asyncio.sleep(self._interval)

        self._task = asyncio.create_task(_loop())
        return self

    async def stop(self):
        """Cancel the background polling task."""
        if self._task:
            self._task.cancel()  # type: ignore[union-attr]
            try:
                await self._task  # type: ignore[union-attr]
            except Exception:
                pass

    async def __aenter__(self) -> "AsyncHealthPoller":
        return await self.start()

    async def __aexit__(self, *_):
        await self.stop()
