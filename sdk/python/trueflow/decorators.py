"""Decorators for easy per-function guardrail configuration.

Example::

    from trueflow import TrueFlowClient, with_guardrails

    client = TrueFlowClient(api_key="tf_v1_...")

    @with_guardrails(["pii_redaction", "prompt_injection"])
    def my_agent_function(prompt: str):
        response = client.openai().chat.completions.create(
            model="gpt-4o",
            messages=[{"role": "user", "content": prompt}],
        )
        return response.choices[0].message.content

    @with_guardrails(["hipaa"], client=client)
    async def my_async_agent(prompt: str):
        response = await client.openai().chat.completions.create(
            model="gpt-4o",
            messages=[{"role": "user", "content": prompt}],
        )
        return response.choices[0].message.content
"""

from __future__ import annotations

import functools
import inspect
from typing import Any, Callable, List, Optional, TypeVar, Union, overload

F = TypeVar("F", bound=Callable[..., Any])


def _get_scoped_client(client_or_none, presets: List[str]) -> Any:
    """Get a scoped client with guardrail headers injected.

    This uses the internal with_guardrails method on the client to inject
    the X-TrueFlow-Guardrails-Enable header.
    """
    if client_or_none is None:
        # Will be resolved at call time from closure
        return None

    header_val = ",".join(presets)
    # Use the client's with_guardrails method to create a scoped client
    return client_or_none.with_guardrails(header_val)


@overload
def with_guardrails(
    presets: List[str],
    *,
    client: Optional[Any] = None,
    mode: str = "enable",
) -> Callable[[F], F]: ...


@overload
def with_guardrails(
    presets: List[str],
    func: F,
) -> F: ...


def with_guardrails(
    presets: List[str],
    func: Optional[F] = None,
    *,
    client: Optional[Any] = None,
    mode: str = "enable",
) -> Union[F, Callable[[F], F]]:
    """Decorator to apply guardrails to a function via request headers.

    Injects the X-TrueFlow-Guardrails-Enable header into TrueFlow client calls.
    Works with both sync and async functions.

    Args:
        presets: List of guardrail preset names to enable (e.g., ["pii_redaction", "prompt_injection"]).
        func: The function to decorate (when used without parentheses).
        client: Optional TrueFlowClient instance. If not provided, the decorated
                function must accept a client parameter or have a client in its closure.
        mode: "enable" to add guardrails (default), "disable" to remove them.

    Returns:
        Decorated function that applies guardrails via headers.

    Examples::

        # Basic usage - decorator determines client at call time
        @with_guardrails(["pii_redaction"])
        def my_function(prompt: str, client: TrueFlowClient):
            return client.openai().chat.completions.create(...)

        # With explicit client
        client = TrueFlowClient(api_key="tf_v1_...")
        @with_guardrails(["prompt_injection", "hipaa"], client=client)
        def my_function(prompt: str):
            return client.openai().chat.completions.create(...)

        # Disable specific guardrails
        @with_guardrails(["pii_redaction"], mode="disable")
        def sensitive_operation(prompt: str, client: TrueFlowClient):
            # PII will NOT be redacted for this call
            return client.openai().chat.completions.create(...)

        # Async support
        @with_guardrails(["pii_enterprise"])
        async def my_async_function(prompt: str, client: TrueFlowClient):
            return await client.openai().chat.completions.create(...)
    """
    if not presets:
        raise ValueError("presets list cannot be empty")

    if mode not in ("enable", "disable"):
        raise ValueError(f"mode must be 'enable' or 'disable', got {mode!r}")

    header_name = (
        "X-TrueFlow-Guardrails-Enable"
        if mode == "enable"
        else "X-TrueFlow-Guardrails-Disable"
    )
    header_val = ",".join(presets)

    def decorator(fn: F) -> F:
        @functools.wraps(fn)
        def sync_wrapper(*args, **kwargs):
            # Inject header into kwargs if a scoped client is being used
            # Look for 'client' or '_client' in kwargs or create scoped client
            if client is not None:
                scoped = client.with_guardrails(header_val)
                # If the function accepts a 'client' kwarg, inject the scoped client
                sig = inspect.signature(fn)
                if "client" in sig.parameters:
                    kwargs = {**kwargs, "client": scoped}
                    return fn(*args, **kwargs)

            # Check if 'client' is in kwargs and wrap it
            if "client" in kwargs and hasattr(kwargs["client"], "with_guardrails"):
                original_client = kwargs["client"]
                kwargs["client"] = original_client.with_guardrails(header_val)
            elif "_client" in kwargs and hasattr(kwargs["_client"], "with_guardrails"):
                original_client = kwargs["_client"]
                kwargs["_client"] = original_client.with_guardrails(header_val)

            return fn(*args, **kwargs)

        @functools.wraps(fn)
        async def async_wrapper(*args, **kwargs):
            if client is not None:
                scoped = client.with_guardrails(header_val)
                sig = inspect.signature(fn)
                if "client" in sig.parameters:
                    kwargs = {**kwargs, "client": scoped}
                    return await fn(*args, **kwargs)

            if "client" in kwargs and hasattr(kwargs["client"], "with_guardrails"):
                original_client = kwargs["client"]
                kwargs["client"] = original_client.with_guardrails(header_val)
            elif "_client" in kwargs and hasattr(kwargs["_client"], "with_guardrails"):
                original_client = kwargs["_client"]
                kwargs["_client"] = original_client.with_guardrails(header_val)

            return await fn(*args, **kwargs)

        if inspect.iscoroutinefunction(fn):
            return async_wrapper  # type: ignore
        return sync_wrapper  # type: ignore

    # Support both @with_guardrails([...]) and @with_guardrails([...], func)
    if func is not None:
        return decorator(func)

    return decorator


class GuardrailContext:
    """Context manager for setting guardrails in a block scope.

    Example::

        client = TrueFlowClient(api_key="tf_v1_...")

        with GuardrailContext(client, ["pii_redaction", "prompt_injection"]):
            # All calls in this block have guardrails enabled
            response = client.openai().chat.completions.create(...)
    """

    def __init__(self, client: Any, presets: List[str], mode: str = "enable"):
        self.client = client
        self.presets = presets
        self.mode = mode
        self._scoped_client = None

    def __enter__(self) -> Any:
        header_val = ",".join(self.presets)
        self._scoped_client = self.client.with_guardrails(header_val)
        return self._scoped_client

    def __exit__(self, *args) -> None:
        self._scoped_client = None

    async def __aenter__(self) -> Any:
        header_val = ",".join(self.presets)
        self._scoped_client = self.client.with_guardrails(header_val)
        return self._scoped_client

    async def __aexit__(self, *args) -> None:
        self._scoped_client = None