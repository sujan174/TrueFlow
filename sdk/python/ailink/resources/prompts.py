"""Resource for prompt management (CRUD, versioning, deployment, rendering).

Matches the gateway API at ``/api/v1/prompts/*``.

.. rubric:: Quick start

::

    admin = TrueFlowClient.admin(admin_key="...")

    # Create a prompt
    prompt = admin.prompts.create(name="Customer Support Agent")

    # Publish a version
    admin.prompts.create_version(
        prompt["id"],
        model="gpt-4o",
        messages=[
            {"role": "system", "content": "You help {{user_name}} with {{topic}}."},
            {"role": "user", "content": "{{question}}"},
        ],
    )

    # Deploy v1 to production
    admin.prompts.deploy(prompt["id"], version=1, label="production")

    # Render for use with OpenAI
    payload = admin.prompts.render(
        "customer-support-agent",
        variables={"user_name": "Alice", "topic": "billing", "question": "Where is my invoice?"},
        label="production",
    )
    # payload is ready to pass to openai.chat.completions.create(**payload)
"""

from typing import Any, Dict, List, Optional

import hashlib
import json
import time

from ..exceptions import raise_for_status


def _cache_key(
    slug: str,
    label: Optional[str],
    version: Optional[int],
    variables: Optional[Dict[str, Any]],
) -> str:
    """Build a deterministic cache key from render parameters."""
    var_hash = ""
    if variables:
        var_hash = hashlib.md5(
            json.dumps(variables, sort_keys=True).encode()
        ).hexdigest()
    return f"{slug}:{label or ''}:{version or ''}:{var_hash}"


class PromptsResource:
    """Management API resource for prompt management.

    Provides CRUD operations, versioning, label-based deployment,
    folder organisation, and variable rendering.
    """

    def __init__(self, client, *, cache_ttl: int = 60) -> None:
        self._client = client
        self._cache: Dict[str, Any] = {}
        self._cache_ts: Dict[str, float] = {}
        self._cache_ttl = cache_ttl

    # ── Prompt CRUD ────────────────────────────────────────────

    def list(self, *, folder: Optional[str] = None) -> List[Dict[str, Any]]:
        """List all prompts, optionally filtered by folder.

        Args:
            folder: Filter to a specific folder path (e.g. ``"/production"``).
        """
        params: Dict[str, Any] = {}
        if folder is not None:
            params["folder"] = folder
        resp = self._client._http.get("/api/v1/prompts", params=params)
        raise_for_status(resp)
        return resp.json()

    def create(
        self,
        name: str,
        *,
        slug: Optional[str] = None,
        description: Optional[str] = None,
        folder: Optional[str] = None,
        tags: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        """Create a new prompt.

        Args:
            name: Human-readable prompt name.
            slug: URL-safe identifier (auto-generated from name if omitted).
            description: Optional description.
            folder: Folder path for organisation (default ``"/"``).
            tags: Arbitrary JSON metadata.

        Returns:
            Created prompt object with ``id``, ``name``, ``slug``.
        """
        payload: Dict[str, Any] = {"name": name}
        if slug is not None:
            payload["slug"] = slug
        if description is not None:
            payload["description"] = description
        if folder is not None:
            payload["folder"] = folder
        if tags is not None:
            payload["tags"] = tags
        resp = self._client._http.post("/api/v1/prompts", json=payload)
        raise_for_status(resp)
        return resp.json()

    def get(self, prompt_id: str) -> Dict[str, Any]:
        """Get a prompt and its versions.

        Args:
            prompt_id: UUID of the prompt.
        """
        resp = self._client._http.get(f"/api/v1/prompts/{prompt_id}")
        raise_for_status(resp)
        return resp.json()

    def update(
        self,
        prompt_id: str,
        name: str,
        *,
        description: Optional[str] = None,
        folder: Optional[str] = None,
        tags: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        """Update prompt metadata.

        Args:
            prompt_id: UUID of the prompt.
            name: New name.
            description: Updated description.
            folder: Updated folder path.
            tags: Updated tags.
        """
        payload: Dict[str, Any] = {"name": name}
        if description is not None:
            payload["description"] = description
        if folder is not None:
            payload["folder"] = folder
        if tags is not None:
            payload["tags"] = tags
        resp = self._client._http.put(f"/api/v1/prompts/{prompt_id}", json=payload)
        raise_for_status(resp)
        return resp.json()

    def delete(self, prompt_id: str) -> Dict[str, Any]:
        """Soft-delete a prompt.

        Args:
            prompt_id: UUID of the prompt.
        """
        resp = self._client._http.delete(f"/api/v1/prompts/{prompt_id}")
        raise_for_status(resp)
        return resp.json()

    # ── Versions ───────────────────────────────────────────────

    def list_versions(self, prompt_id: str) -> List[Dict[str, Any]]:
        """List all versions for a prompt (newest first).

        Args:
            prompt_id: UUID of the prompt.
        """
        resp = self._client._http.get(f"/api/v1/prompts/{prompt_id}/versions")
        raise_for_status(resp)
        return resp.json()

    def create_version(
        self,
        prompt_id: str,
        *,
        model: str,
        messages: Any,
        temperature: Optional[float] = None,
        max_tokens: Optional[int] = None,
        top_p: Optional[float] = None,
        tools: Optional[Any] = None,
        commit_message: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Publish a new version of a prompt.

        Args:
            prompt_id: UUID of the prompt.
            model: Model identifier (e.g. ``"gpt-4o"``).
            messages: OpenAI-format messages array (may contain ``{{variable}}`` placeholders).
            temperature: Sampling temperature.
            max_tokens: Max completion tokens.
            top_p: Nucleus sampling parameter.
            tools: Tool/function definitions (OpenAI format).
            commit_message: Human-readable change description.

        Returns:
            Created version with ``id``, ``version`` number.
        """
        payload: Dict[str, Any] = {"model": model, "messages": messages}
        if temperature is not None:
            payload["temperature"] = temperature
        if max_tokens is not None:
            payload["max_tokens"] = max_tokens
        if top_p is not None:
            payload["top_p"] = top_p
        if tools is not None:
            payload["tools"] = tools
        if commit_message is not None:
            payload["commit_message"] = commit_message
        resp = self._client._http.post(f"/api/v1/prompts/{prompt_id}/versions", json=payload)
        raise_for_status(resp)
        return resp.json()

    def get_version(self, prompt_id: str, version: int) -> Dict[str, Any]:
        """Get a specific version of a prompt.

        Args:
            prompt_id: UUID of the prompt.
            version: Version number.
        """
        resp = self._client._http.get(f"/api/v1/prompts/{prompt_id}/versions/{version}")
        raise_for_status(resp)
        return resp.json()

    # ── Deployment ─────────────────────────────────────────────

    def deploy(self, prompt_id: str, *, version: int, label: str) -> Dict[str, Any]:
        """Deploy a version to a label (e.g. ``"production"``, ``"staging"``).

        Atomically promotes a version — the previous holder of the label
        is demoted. Use labels to manage environment-specific deployments
        without changing application code.

        Args:
            prompt_id: UUID of the prompt.
            version: Version number to promote.
            label: Label name (e.g. ``"production"``).
        """
        payload = {"version": version, "label": label}
        resp = self._client._http.post(f"/api/v1/prompts/{prompt_id}/deploy", json=payload)
        raise_for_status(resp)
        return resp.json()

    # ── Rendering ──────────────────────────────────────────────

    def render(
        self,
        slug: str,
        *,
        variables: Optional[Dict[str, Any]] = None,
        label: Optional[str] = None,
        version: Optional[int] = None,
    ) -> Dict[str, Any]:
        """Render a prompt with variable substitution.

        Returns an OpenAI-compatible payload (``model``, ``messages``,
        ``temperature``, etc.) ready to pass directly to
        ``openai.chat.completions.create(**payload)``.

        Resolution order:
        1. Exact ``version`` number (if provided).
        2. Version with matching ``label`` (e.g. ``"production"``).
        3. Latest version.

        Args:
            slug: URL-safe prompt identifier.
            variables: ``{{placeholder}}`` replacements.
            label: Label to resolve (e.g. ``"production"``).
            version: Exact version number.

        Returns:
            Rendered prompt payload with ``model``, ``messages``, ``version``,
            ``prompt_id``, ``prompt_slug``, and optional ``temperature``,
            ``max_tokens``, ``top_p``, ``tools``.
        """
        payload: Dict[str, Any] = {}
        if variables:
            payload["variables"] = variables
        if label is not None:
            payload["label"] = label
        if version is not None:
            payload["version"] = version

        # Check cache
        key = _cache_key(slug, label, version, variables)
        if key in self._cache and (time.monotonic() - self._cache_ts[key]) < self._cache_ttl:
            return self._cache[key]

        resp = self._client._http.post(f"/api/v1/prompts/by-slug/{slug}/render", json=payload)
        raise_for_status(resp)
        result = resp.json()

        # Store in cache
        self._cache[key] = result
        self._cache_ts[key] = time.monotonic()
        return result

    # ── Cache Management ──────────────────────────────────────

    def clear_cache(self) -> None:
        """Clear all cached rendered prompts."""
        self._cache.clear()
        self._cache_ts.clear()

    def invalidate(self, slug: str) -> None:
        """Invalidate all cache entries for a specific prompt slug."""
        keys_to_remove = [k for k in self._cache if k.startswith(f"{slug}:")]
        for k in keys_to_remove:
            del self._cache[k]
            del self._cache_ts[k]

    # ── Folders ────────────────────────────────────────────────

    def list_folders(self) -> List[str]:
        """List all unique folder paths across prompts."""
        resp = self._client._http.get("/api/v1/prompts/folders")
        raise_for_status(resp)
        return resp.json()


class AsyncPromptsResource:
    """Async variant of PromptsResource."""

    def __init__(self, client, *, cache_ttl: int = 60) -> None:
        self._client = client
        self._cache: Dict[str, Any] = {}
        self._cache_ts: Dict[str, float] = {}
        self._cache_ttl = cache_ttl

    async def list(self, *, folder: Optional[str] = None) -> List[Dict[str, Any]]:
        params: Dict[str, Any] = {}
        if folder is not None:
            params["folder"] = folder
        resp = await self._client._http.get("/api/v1/prompts", params=params)
        raise_for_status(resp)
        return resp.json()

    async def create(self, name: str, *, slug: Optional[str] = None, description: Optional[str] = None, folder: Optional[str] = None, tags: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        payload: Dict[str, Any] = {"name": name}
        if slug is not None:
            payload["slug"] = slug
        if description is not None:
            payload["description"] = description
        if folder is not None:
            payload["folder"] = folder
        if tags is not None:
            payload["tags"] = tags
        resp = await self._client._http.post("/api/v1/prompts", json=payload)
        raise_for_status(resp)
        return resp.json()

    async def get(self, prompt_id: str) -> Dict[str, Any]:
        resp = await self._client._http.get(f"/api/v1/prompts/{prompt_id}")
        raise_for_status(resp)
        return resp.json()

    async def update(self, prompt_id: str, name: str, *, description: Optional[str] = None, folder: Optional[str] = None, tags: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        payload: Dict[str, Any] = {"name": name}
        if description is not None:
            payload["description"] = description
        if folder is not None:
            payload["folder"] = folder
        if tags is not None:
            payload["tags"] = tags
        resp = await self._client._http.put(f"/api/v1/prompts/{prompt_id}", json=payload)
        raise_for_status(resp)
        return resp.json()

    async def delete(self, prompt_id: str) -> Dict[str, Any]:
        resp = await self._client._http.delete(f"/api/v1/prompts/{prompt_id}")
        raise_for_status(resp)
        return resp.json()

    async def list_versions(self, prompt_id: str) -> List[Dict[str, Any]]:
        resp = await self._client._http.get(f"/api/v1/prompts/{prompt_id}/versions")
        raise_for_status(resp)
        return resp.json()

    async def create_version(self, prompt_id: str, *, model: str, messages: Any, temperature: Optional[float] = None, max_tokens: Optional[int] = None, top_p: Optional[float] = None, tools: Optional[Any] = None, commit_message: Optional[str] = None) -> Dict[str, Any]:
        payload: Dict[str, Any] = {"model": model, "messages": messages}
        if temperature is not None:
            payload["temperature"] = temperature
        if max_tokens is not None:
            payload["max_tokens"] = max_tokens
        if top_p is not None:
            payload["top_p"] = top_p
        if tools is not None:
            payload["tools"] = tools
        if commit_message is not None:
            payload["commit_message"] = commit_message
        resp = await self._client._http.post(f"/api/v1/prompts/{prompt_id}/versions", json=payload)
        raise_for_status(resp)
        return resp.json()

    async def get_version(self, prompt_id: str, version: int) -> Dict[str, Any]:
        resp = await self._client._http.get(f"/api/v1/prompts/{prompt_id}/versions/{version}")
        raise_for_status(resp)
        return resp.json()

    async def deploy(self, prompt_id: str, *, version: int, label: str) -> Dict[str, Any]:
        payload = {"version": version, "label": label}
        resp = await self._client._http.post(f"/api/v1/prompts/{prompt_id}/deploy", json=payload)
        raise_for_status(resp)
        return resp.json()

    async def render(self, slug: str, *, variables: Optional[Dict[str, Any]] = None, label: Optional[str] = None, version: Optional[int] = None) -> Dict[str, Any]:
        payload: Dict[str, Any] = {}
        if variables:
            payload["variables"] = variables
        if label is not None:
            payload["label"] = label
        if version is not None:
            payload["version"] = version

        key = _cache_key(slug, label, version, variables)
        if key in self._cache and (time.monotonic() - self._cache_ts[key]) < self._cache_ttl:
            return self._cache[key]

        resp = await self._client._http.post(f"/api/v1/prompts/by-slug/{slug}/render", json=payload)
        raise_for_status(resp)
        result = resp.json()

        self._cache[key] = result
        self._cache_ts[key] = time.monotonic()
        return result

    def clear_cache(self) -> None:
        self._cache.clear()
        self._cache_ts.clear()

    def invalidate(self, slug: str) -> None:
        keys_to_remove = [k for k in self._cache if k.startswith(f"{slug}:")]
        for k in keys_to_remove:
            del self._cache[k]
            del self._cache_ts[k]

    async def list_folders(self) -> List[str]:
        resp = await self._client._http.get("/api/v1/prompts/folders")
        raise_for_status(resp)
        return resp.json()
