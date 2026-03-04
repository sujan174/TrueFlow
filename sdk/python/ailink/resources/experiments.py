"""Resource for experiment tracking (A/B testing for prompts and models).

Run A/B experiments to compare models, prompts, or routing strategies::

    admin = TrueFlowClient.admin(admin_key="...")

    # Create an experiment
    exp = admin.experiments.create(
        name="gpt4o-vs-claude",
        variants=[
            {"name": "control", "weight": 50, "model": "gpt-4o"},
            {"name": "treatment", "weight": 50, "model": "claude-3-5-sonnet-20241022"},
        ],
    )

    # Check results
    results = admin.experiments.results(exp["id"])

    # Stop when done
    admin.experiments.stop(exp["id"])
"""

from typing import Any, Dict, List, Optional

from ..exceptions import raise_for_status


class ExperimentsResource:
    """Management API resource for experiment tracking.

    Run A/B experiments to compare models, prompts, or routing strategies.
    Experiments create Split policies under the hood — variants are randomly
    assigned per-request using deterministic hashing.
    """

    def __init__(self, client) -> None:
        self._client = client

    def create(
        self,
        name: str,
        variants: List[Dict[str, Any]],
        *,
        condition: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        """Create an A/B experiment.

        Args:
            name: Unique experiment name (e.g. ``"gpt4o-vs-claude"``).
            variants: List of variant dicts. Each must have ``name`` (str),
                ``weight`` (int), and either ``model`` (str) or
                ``set_body_fields`` (dict of overrides).
            condition: Optional condition tree for scoping which requests
                enter the experiment. Default: all requests.

        Returns:
            Created experiment with ``id``, ``name``, ``status``, ``variants``.
        """
        payload: Dict[str, Any] = {"name": name, "variants": variants}
        if condition is not None:
            payload["condition"] = condition
        resp = self._client._http.post("/api/v1/experiments", json=payload)
        raise_for_status(resp)
        return resp.json()

    def list(self) -> List[Dict[str, Any]]:
        """List all running experiments."""
        resp = self._client._http.get("/api/v1/experiments")
        raise_for_status(resp)
        return resp.json()

    def get(self, experiment_id: str) -> Dict[str, Any]:
        """Get an experiment with its latest analytics.

        Args:
            experiment_id: UUID of the experiment (also the policy ID).
        """
        resp = self._client._http.get(f"/api/v1/experiments/{experiment_id}")
        raise_for_status(resp)
        return resp.json()

    def results(self, experiment_id: str) -> Dict[str, Any]:
        """Get per-variant results for an experiment.

        Returns variant-level metrics: requests, latency, cost, tokens,
        error count, and error rate.

        Args:
            experiment_id: UUID of the experiment.
        """
        resp = self._client._http.get(f"/api/v1/experiments/{experiment_id}/results")
        raise_for_status(resp)
        return resp.json()

    def stop(self, experiment_id: str) -> Dict[str, Any]:
        """Stop a running experiment.

        Args:
            experiment_id: UUID of the experiment.
        """
        resp = self._client._http.post(f"/api/v1/experiments/{experiment_id}/stop")
        raise_for_status(resp)
        return resp.json()

    def update(
        self,
        experiment_id: str,
        variants: List[Dict[str, Any]],
    ) -> Dict[str, Any]:
        """Update variant weights for a running experiment.

        Args:
            experiment_id: UUID of the experiment.
            variants: Updated variant list with new weights.
        """
        payload = {"variants": variants}
        resp = self._client._http.put(f"/api/v1/experiments/{experiment_id}", json=payload)
        raise_for_status(resp)
        return resp.json()


class AsyncExperimentsResource:
    """Async variant of ExperimentsResource."""

    def __init__(self, client) -> None:
        self._client = client

    async def create(self, name: str, variants: List[Dict[str, Any]], *, condition: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        payload: Dict[str, Any] = {"name": name, "variants": variants}
        if condition is not None:
            payload["condition"] = condition
        resp = await self._client._http.post("/api/v1/experiments", json=payload)
        raise_for_status(resp)
        return resp.json()

    async def list(self) -> List[Dict[str, Any]]:
        resp = await self._client._http.get("/api/v1/experiments")
        raise_for_status(resp)
        return resp.json()

    async def get(self, experiment_id: str) -> Dict[str, Any]:
        resp = await self._client._http.get(f"/api/v1/experiments/{experiment_id}")
        raise_for_status(resp)
        return resp.json()

    async def results(self, experiment_id: str) -> Dict[str, Any]:
        resp = await self._client._http.get(f"/api/v1/experiments/{experiment_id}/results")
        raise_for_status(resp)
        return resp.json()

    async def stop(self, experiment_id: str) -> Dict[str, Any]:
        resp = await self._client._http.post(f"/api/v1/experiments/{experiment_id}/stop")
        raise_for_status(resp)
        return resp.json()

    async def update(self, experiment_id: str, variants: List[Dict[str, Any]]) -> Dict[str, Any]:
        payload = {"variants": variants}
        resp = await self._client._http.put(f"/api/v1/experiments/{experiment_id}", json=payload)
        raise_for_status(resp)
        return resp.json()
