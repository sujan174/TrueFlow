"""
Feature 10: Batches API resource.

Proxies requests to OpenAI's /v1/batches endpoint through the TrueFlow gateway,
with full credential injection, audit logging, and token-based access control.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Dict, Optional

from ..exceptions import raise_for_status

if TYPE_CHECKING:
    from ..client import TrueFlowClient, AsyncClient


class BatchesResource:
    """Synchronous Batches API resource.

    Exposes the same interface as the OpenAI SDK's `client.batches` resource.
    All requests are routed through the TrueFlow gateway for auditability and
    policy enforcement.

    Example::

        batch = client.batches.create(
            input_file_id="file-abc123",
            endpoint="/v1/chat/completions",
            completion_window="24h",
        )
        print(batch["id"], batch["status"])
    """

    def __init__(self, client: "TrueFlowClient") -> None:
        self._client = client

    def create(
        self,
        *,
        input_file_id: str,
        endpoint: str,
        completion_window: str = "24h",
        metadata: Optional[Dict[str, str]] = None,
    ) -> Dict[str, Any]:
        """Create a new batch job.

        Args:
            input_file_id: ID of the uploaded JSONL file containing batch requests.
            endpoint: Target endpoint, e.g. ``"/v1/chat/completions"``.
            completion_window: Maximum time window, e.g. ``"24h"``.
            metadata: Optional key-value metadata for the batch.
        """
        body: Dict[str, Any] = {
            "input_file_id": input_file_id,
            "endpoint": endpoint,
            "completion_window": completion_window,
        }
        if metadata is not None:
            body["metadata"] = metadata
        resp = self._client._http.post("/v1/batches", json=body)
        raise_for_status(resp)
        return resp.json()

    def retrieve(self, batch_id: str) -> Dict[str, Any]:
        """Retrieve a batch job by ID."""
        resp = self._client._http.get(f"/v1/batches/{batch_id}")
        raise_for_status(resp)
        return resp.json()

    def list(
        self,
        *,
        after: Optional[str] = None,
        limit: int = 20,
    ) -> Dict[str, Any]:
        """List batch jobs."""
        params: Dict[str, Any] = {"limit": limit}
        if after:
            params["after"] = after
        resp = self._client._http.get("/v1/batches", params=params)
        raise_for_status(resp)
        return resp.json()

    def cancel(self, batch_id: str) -> Dict[str, Any]:
        """Cancel a pending or in-progress batch job."""
        resp = self._client._http.post(f"/v1/batches/{batch_id}/cancel", json={})
        raise_for_status(resp)
        return resp.json()


class AsyncBatchesResource:
    """Asynchronous Batches API resource.

    Drop-in async equivalent of :class:`BatchesResource`.

    Example::

        batch = await client.batches.create(
            input_file_id="file-abc123",
            endpoint="/v1/chat/completions",
        )
    """

    def __init__(self, client: "AsyncClient") -> None:
        self._client = client

    async def create(
        self,
        *,
        input_file_id: str,
        endpoint: str,
        completion_window: str = "24h",
        metadata: Optional[Dict[str, str]] = None,
    ) -> Dict[str, Any]:
        body: Dict[str, Any] = {
            "input_file_id": input_file_id,
            "endpoint": endpoint,
            "completion_window": completion_window,
        }
        if metadata is not None:
            body["metadata"] = metadata
        resp = await self._client._http.post("/v1/batches", json=body)
        raise_for_status(resp)
        return resp.json()

    async def retrieve(self, batch_id: str) -> Dict[str, Any]:
        resp = await self._client._http.get(f"/v1/batches/{batch_id}")
        raise_for_status(resp)
        return resp.json()

    async def list(
        self,
        *,
        after: Optional[str] = None,
        limit: int = 20,
    ) -> Dict[str, Any]:
        params: Dict[str, Any] = {"limit": limit}
        if after:
            params["after"] = after
        resp = await self._client._http.get("/v1/batches", params=params)
        raise_for_status(resp)
        return resp.json()

    async def cancel(self, batch_id: str) -> Dict[str, Any]:
        resp = await self._client._http.post(f"/v1/batches/{batch_id}/cancel", json={})
        raise_for_status(resp)
        return resp.json()
