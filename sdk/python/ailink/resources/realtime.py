"""Realtime WebSocket session proxy — connects to the TrueFlow Realtime gateway."""

from __future__ import annotations

import json
from typing import Any, AsyncIterator, Dict, Iterator, Optional

try:
    import websockets
    from websockets.sync.client import connect as sync_connect
    from websockets.asyncio.client import connect as async_connect
except ImportError:
    websockets = None  # type: ignore

from ..exceptions import TrueFlowError


# ── Sync Realtime Session ─────────────────────────────────────

class RealtimeSession:
    """Synchronous Realtime WebSocket session proxied through TrueFlow.

    Usage::

        import json
        from trueflow import TrueFlowClient

        client = TrueFlowClient.admin(admin_key="...admin key...", gateway_url="ws://localhost:8080")

        with client.realtime.connect(model="gpt-4o-realtime-preview") as session:
            # Send a session.update event
            session.send({
                "type": "session.update",
                "session": {"modalities": ["text"], "instructions": "You are a helpful assistant."}
            })

            # Send a conversation input
            session.send({
                "type": "conversation.item.create",
                "item": {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "Hello!"}]
                }
            })
            session.send({"type": "response.create"})

            # Receive events until response.done
            for event in session:
                print(event["type"])
                if event["type"] == "response.done":
                    break
    """

    def __init__(self, ws, token: str, model: str):
        self._ws = ws
        self._token = token
        self._model = model

    def send(self, event: Dict[str, Any]) -> None:
        """Send a Realtime API event (dict is JSON-serialized)."""
        self._ws.send(json.dumps(event))

    def recv(self) -> Dict[str, Any]:
        """Receive the next event from the Realtime API."""
        msg = self._ws.recv()
        return json.loads(msg)

    def __iter__(self) -> Iterator[Dict[str, Any]]:
        """Iterate over incoming events until the connection closes."""
        try:
            while True:
                yield self.recv()
        except Exception:
            return

    def close(self) -> None:
        """Close the session."""
        self._ws.close()

    def __enter__(self) -> "RealtimeSession":
        return self

    def __exit__(self, *_) -> None:
        self.close()


class RealtimeResource:
    """Manage Realtime WebSocket sessions via the TrueFlow gateway.

    Create sessions using ``connect()`` as a context manager::

        with client.realtime.connect(model="gpt-4o-realtime-preview") as session:
            session.send({"type": "session.update", ...})
            event = session.recv()
    """

    def __init__(self, client):
        self._client = client

    def connect(
        self,
        model: str = "gpt-4o-realtime-preview-2024-12-17",
        additional_headers: Optional[Dict[str, str]] = None,
    ) -> RealtimeSession:
        """Open a synchronous Realtime WebSocket session.

        Args:
            model: The realtime model to use.
            additional_headers: Extra headers to forward with the upgrade request.

        Returns:
            A :class:`RealtimeSession` context manager.
        """
        if websockets is None:
            raise ImportError(
                "The 'websockets' package is required for client.realtime. "
                "Install it with: pip install trueflow[realtime]\n"
                "Or standalone: pip install 'websockets>=12'"
            )
        api_key = getattr(self._client, "api_key", None) or getattr(self._client, "_token", None)
        gateway_url = self._client.gateway_url.replace("http://", "ws://").replace("https://", "wss://")
        ws_url = f"{gateway_url}/v1/realtime?model={model}"

        headers = {"Authorization": f"Bearer {api_key}"}
        if additional_headers:
            headers.update(additional_headers)

        ws = sync_connect(ws_url, additional_headers=headers)
        return RealtimeSession(ws, token=api_key, model=model)


# ── Async Realtime Session ────────────────────────────────────

class AsyncRealtimeSession:
    """Asynchronous Realtime WebSocket session proxied through TrueFlow.

    Usage::

        import asyncio

        async def main():
            async with client.realtime.connect(model="gpt-4o-realtime-preview") as session:
                await session.send({"type": "session.update", "session": {...}})
                async for event in session:
                    print(event["type"])
                    if event["type"] == "response.done":
                        break

        asyncio.run(main())
    """

    def __init__(self, ws, token: str, model: str):
        self._ws = ws
        self._token = token
        self._model = model

    async def send(self, event: Dict[str, Any]) -> None:
        """Send a Realtime API event."""
        await self._ws.send(json.dumps(event))

    async def recv(self) -> Dict[str, Any]:
        """Receive the next event."""
        msg = await self._ws.recv()
        return json.loads(msg)

    async def __aiter__(self) -> AsyncIterator[Dict[str, Any]]:
        """Async iteration over incoming events until the connection closes."""
        try:
            while True:
                yield await self.recv()
        except Exception:
            return

    async def close(self) -> None:
        """Close the session."""
        await self._ws.close()

    async def __aenter__(self) -> "AsyncRealtimeSession":
        return self

    async def __aexit__(self, *_) -> None:
        await self.close()


class _AsyncRealtimeConnectCtx:
    """Context manager that opens and returns an AsyncRealtimeSession."""

    def __init__(self, client, model: str, extra_headers: Dict[str, str]):
        self._client = client
        self._model = model
        self._extra_headers = extra_headers
        self._session: Optional[AsyncRealtimeSession] = None

    async def __aenter__(self) -> AsyncRealtimeSession:
        if websockets is None:
            raise ImportError("Install websockets: pip install trueflow[realtime]")
        api_key = getattr(self._client, "api_key", None) or getattr(self._client, "_token", None)
        gateway_url = self._client.gateway_url.replace("http://", "ws://").replace("https://", "wss://")
        ws_url = f"{gateway_url}/v1/realtime?model={self._model}"
        headers = {"Authorization": f"Bearer {api_key}"}
        headers.update(self._extra_headers)
        ws = await async_connect(ws_url, additional_headers=headers)
        self._session = AsyncRealtimeSession(ws, token=api_key, model=self._model)
        return self._session

    async def __aexit__(self, *_) -> None:
        if self._session:
            await self._session.close()


class AsyncRealtimeResource:
    """Async Realtime WebSocket sessions via TrueFlow gateway."""

    def __init__(self, client):
        self._client = client

    def connect(
        self,
        model: str = "gpt-4o-realtime-preview-2024-12-17",
        additional_headers: Optional[Dict[str, str]] = None,
    ) -> _AsyncRealtimeConnectCtx:
        """Open an async Realtime WebSocket session.

        Must be used as an async context manager::

            async with client.realtime.connect(...) as session:
                await session.send(...)

        Returns:
            An async context manager yielding :class:`AsyncRealtimeSession`.
        """
        if websockets is None:
            raise ImportError("Install websockets: pip install trueflow[realtime]")
        return _AsyncRealtimeConnectCtx(
            self._client,
            model=model,
            extra_headers=additional_headers or {},
        )
