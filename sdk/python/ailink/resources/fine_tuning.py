"""
Feature 10: Fine-tuning API resource.

Proxies requests to OpenAI's /v1/fine_tuning endpoint through the TrueFlow
gateway, with credential injection, audit logging, and policy enforcement.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Dict, Optional

from ..exceptions import raise_for_status

if TYPE_CHECKING:
    from ..client import TrueFlowClient, AsyncClient


class FineTuningResource:
    """Synchronous Fine-tuning API resource.

    Exposes the same interface as the OpenAI SDK's ``client.fine_tuning.jobs``
    resource. All requests are routed through the TrueFlow gateway.

    Example::

        job = client.fine_tuning.create_job(
            model="gpt-4o-mini",
            training_file="file-xyz",
        )
        print(job["id"], job["status"])
    """

    def __init__(self, client: "TrueFlowClient") -> None:
        self._client = client

    # ── Jobs ──────────────────────────────────────────────────

    def create_job(
        self,
        *,
        model: str,
        training_file: str,
        validation_file: Optional[str] = None,
        hyperparameters: Optional[Dict[str, Any]] = None,
        suffix: Optional[str] = None,
        seed: Optional[int] = None,
    ) -> Dict[str, Any]:
        """Create a fine-tuning job."""
        body: Dict[str, Any] = {
            "model": model,
            "training_file": training_file,
        }
        if validation_file is not None:
            body["validation_file"] = validation_file
        if hyperparameters is not None:
            body["hyperparameters"] = hyperparameters
        if suffix is not None:
            body["suffix"] = suffix
        if seed is not None:
            body["seed"] = seed
        resp = self._client._http.post("/v1/fine_tuning/jobs", json=body)
        raise_for_status(resp)
        return resp.json()

    def list_jobs(
        self,
        *,
        after: Optional[str] = None,
        limit: int = 20,
    ) -> Dict[str, Any]:
        """List fine-tuning jobs."""
        params: Dict[str, Any] = {"limit": limit}
        if after:
            params["after"] = after
        resp = self._client._http.get("/v1/fine_tuning/jobs", params=params)
        raise_for_status(resp)
        return resp.json()

    def get_job(self, fine_tuning_job_id: str) -> Dict[str, Any]:
        """Retrieve a fine-tuning job by ID."""
        resp = self._client._http.get(f"/v1/fine_tuning/jobs/{fine_tuning_job_id}")
        raise_for_status(resp)
        return resp.json()

    def cancel_job(self, fine_tuning_job_id: str) -> Dict[str, Any]:
        """Cancel a running fine-tuning job."""
        resp = self._client._http.post(
            f"/v1/fine_tuning/jobs/{fine_tuning_job_id}/cancel", json={}
        )
        raise_for_status(resp)
        return resp.json()

    # ── Events & Checkpoints ──────────────────────────────────

    def list_events(
        self,
        fine_tuning_job_id: str,
        *,
        after: Optional[str] = None,
        limit: int = 20,
    ) -> Dict[str, Any]:
        """List events for a fine-tuning job."""
        params: Dict[str, Any] = {"limit": limit}
        if after:
            params["after"] = after
        resp = self._client._http.get(
            f"/v1/fine_tuning/jobs/{fine_tuning_job_id}/events", params=params
        )
        raise_for_status(resp)
        return resp.json()

    def list_checkpoints(
        self,
        fine_tuning_job_id: str,
        *,
        after: Optional[str] = None,
        limit: int = 10,
    ) -> Dict[str, Any]:
        """List checkpoints for a fine-tuning job."""
        params: Dict[str, Any] = {"limit": limit}
        if after:
            params["after"] = after
        resp = self._client._http.get(
            f"/v1/fine_tuning/jobs/{fine_tuning_job_id}/checkpoints", params=params
        )
        raise_for_status(resp)
        return resp.json()


class AsyncFineTuningResource:
    """Asynchronous Fine-tuning API resource.

    Drop-in async equivalent of :class:`FineTuningResource`.

    Example::

        job = await client.fine_tuning.create_job(
            model="gpt-4o-mini",
            training_file="file-xyz",
        )
    """

    def __init__(self, client: "AsyncClient") -> None:
        self._client = client

    # ── Jobs ──────────────────────────────────────────────────

    async def create_job(
        self,
        *,
        model: str,
        training_file: str,
        validation_file: Optional[str] = None,
        hyperparameters: Optional[Dict[str, Any]] = None,
        suffix: Optional[str] = None,
        seed: Optional[int] = None,
    ) -> Dict[str, Any]:
        body: Dict[str, Any] = {
            "model": model,
            "training_file": training_file,
        }
        if validation_file is not None:
            body["validation_file"] = validation_file
        if hyperparameters is not None:
            body["hyperparameters"] = hyperparameters
        if suffix is not None:
            body["suffix"] = suffix
        if seed is not None:
            body["seed"] = seed
        resp = await self._client._http.post("/v1/fine_tuning/jobs", json=body)
        raise_for_status(resp)
        return resp.json()

    async def list_jobs(
        self,
        *,
        after: Optional[str] = None,
        limit: int = 20,
    ) -> Dict[str, Any]:
        params: Dict[str, Any] = {"limit": limit}
        if after:
            params["after"] = after
        resp = await self._client._http.get("/v1/fine_tuning/jobs", params=params)
        raise_for_status(resp)
        return resp.json()

    async def get_job(self, fine_tuning_job_id: str) -> Dict[str, Any]:
        resp = await self._client._http.get(
            f"/v1/fine_tuning/jobs/{fine_tuning_job_id}"
        )
        raise_for_status(resp)
        return resp.json()

    async def cancel_job(self, fine_tuning_job_id: str) -> Dict[str, Any]:
        resp = await self._client._http.post(
            f"/v1/fine_tuning/jobs/{fine_tuning_job_id}/cancel", json={}
        )
        raise_for_status(resp)
        return resp.json()

    # ── Events & Checkpoints ──────────────────────────────────

    async def list_events(
        self,
        fine_tuning_job_id: str,
        *,
        after: Optional[str] = None,
        limit: int = 20,
    ) -> Dict[str, Any]:
        params: Dict[str, Any] = {"limit": limit}
        if after:
            params["after"] = after
        resp = await self._client._http.get(
            f"/v1/fine_tuning/jobs/{fine_tuning_job_id}/events", params=params
        )
        raise_for_status(resp)
        return resp.json()

    async def list_checkpoints(
        self,
        fine_tuning_job_id: str,
        *,
        after: Optional[str] = None,
        limit: int = 10,
    ) -> Dict[str, Any]:
        params: Dict[str, Any] = {"limit": limit}
        if after:
            params["after"] = after
        resp = await self._client._http.get(
            f"/v1/fine_tuning/jobs/{fine_tuning_job_id}/checkpoints", params=params
        )
        raise_for_status(resp)
        return resp.json()
