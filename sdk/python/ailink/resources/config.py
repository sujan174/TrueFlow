"""Config-as-Code: export and import TrueFlow configuration as YAML/JSON."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Dict, List, Optional, Union

from ..exceptions import raise_for_status


class ConfigResource:
    """Export and import complete TrueFlow configuration (policies + tokens) as YAML or JSON.

    This enables GitOps and infrastructure-as-code workflows where your TrueFlow
    configuration lives in version control alongside your application code.

    Example usage::

        # Export current config to a YAML file
        client.config.export_to_file("trueflow_config.yaml")

        # Edit the file, then re-import:
        client.config.import_from_file("trueflow_config.yaml")

        # Or work with the raw dict
        cfg = client.config.export(format="json")
        cfg["policies"][0]["mode"] = "shadow"
        client.config.import_config(cfg)
    """

    def __init__(self, client):
        self._client = client

    # ── Export ──────────────────────────────────────────────────

    def export(
        self,
        format: str = "yaml",
        project_id: Optional[str] = None,
    ) -> Union[str, Dict[str, Any]]:
        """Export all policies and tokens.

        Args:
            format: ``"yaml"`` (default) returns a YAML string.
                    ``"json"`` returns a Python dict.
            project_id: Optional project scope filter.

        Returns:
            YAML string if ``format="yaml"``, dict if ``format="json"``.
        """
        params: Dict[str, Any] = {"format": format}
        if project_id:
            params["project_id"] = project_id

        resp = self._client._http.get("/api/v1/config/export", params=params)
        raise_for_status(resp)

        if format == "json":
            return resp.json()
        return resp.text

    def export_policies(
        self,
        format: str = "yaml",
        project_id: Optional[str] = None,
    ) -> Union[str, Dict[str, Any]]:
        """Export policies only."""
        params: Dict[str, Any] = {"format": format}
        if project_id:
            params["project_id"] = project_id
        resp = self._client._http.get("/api/v1/config/export/policies", params=params)
        raise_for_status(resp)
        return resp.json() if format == "json" else resp.text

    def export_tokens(
        self,
        format: str = "yaml",
        project_id: Optional[str] = None,
    ) -> Union[str, Dict[str, Any]]:
        """Export tokens only (no credentials)."""
        params: Dict[str, Any] = {"format": format}
        if project_id:
            params["project_id"] = project_id
        resp = self._client._http.get("/api/v1/config/export/tokens", params=params)
        raise_for_status(resp)
        return resp.json() if format == "json" else resp.text

    def export_to_file(
        self,
        path: Union[str, Path],
        format: Optional[str] = None,
        project_id: Optional[str] = None,
    ) -> Path:
        """Export config and save to a file.

        The format is auto-detected from the file extension unless ``format`` is specified.
        ``.yaml`` / ``.yml`` → YAML; ``.json`` → JSON.

        Args:
            path: Destination file path.
            format: ``"yaml"`` or ``"json"``. If omitted, inferred from extension.
            project_id: Optional project scope.

        Returns:
            The resolved ``Path`` object.
        """
        path = Path(path)
        if format is None:
            format = "json" if path.suffix == ".json" else "yaml"

        content = self.export(format=format, project_id=project_id)
        if isinstance(content, dict):
            content = json.dumps(content, indent=2)
        path.write_text(content, encoding="utf-8")
        return path

    # ── Import ──────────────────────────────────────────────────

    def import_config(
        self,
        config: Union[str, Dict[str, Any]],
        project_id: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Import (upsert) configuration from a YAML string or dict.

        Policies and tokens are matched by name. Existing records are updated;
        new records are created. Credentials are never overwritten.

        Args:
            config: YAML string, JSON string, or a dict.
            project_id: Optional project scope (defaults to default project).

        Returns:
            Import summary: ``{"policies_created": N, "policies_updated": M,
            "tokens_created": P, "tokens_updated": Q}``
        """
        if isinstance(config, dict):
            body = json.dumps(config).encode()
            content_type = "application/json"
        else:
            body = config.encode("utf-8")
            content_type = "application/yaml"

        params: Dict[str, Any] = {}
        if project_id:
            params["project_id"] = project_id

        resp = self._client._http.post(
            "/api/v1/config/import",
            content=body,
            headers={"Content-Type": content_type},
            params=params,
        )
        raise_for_status(resp)
        return resp.json()

    def import_from_file(
        self,
        path: Union[str, Path],
        project_id: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Import configuration from a YAML or JSON file.

        Args:
            path: Path to the config file (.yaml, .yml, or .json).
            project_id: Optional project scope.

        Returns:
            Import summary dict.
        """
        path = Path(path)
        content = path.read_text(encoding="utf-8")
        return self.import_config(content, project_id=project_id)


class AsyncConfigResource:
    """Async version of ConfigResource."""

    def __init__(self, client):
        self._client = client

    async def export(
        self,
        format: str = "yaml",
        project_id: Optional[str] = None,
    ) -> Union[str, Dict[str, Any]]:
        """Export all policies and tokens (async)."""
        params: Dict[str, Any] = {"format": format}
        if project_id:
            params["project_id"] = project_id
        resp = await self._client._http.get("/api/v1/config/export", params=params)
        raise_for_status(resp)
        return resp.json() if format == "json" else resp.text

    async def export_to_file(
        self,
        path: Union[str, Path],
        format: Optional[str] = None,
        project_id: Optional[str] = None,
    ) -> Path:
        """Async export to file."""
        path = Path(path)
        if format is None:
            format = "json" if path.suffix == ".json" else "yaml"
        content = await self.export(format=format, project_id=project_id)
        if isinstance(content, dict):
            content = json.dumps(content, indent=2)
        path.write_text(content, encoding="utf-8")
        return path

    async def import_config(
        self,
        config: Union[str, Dict[str, Any]],
        project_id: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Async import from YAML string or dict."""
        if isinstance(config, dict):
            body = json.dumps(config).encode()
            content_type = "application/json"
        else:
            body = config.encode("utf-8")
            content_type = "application/yaml"

        params: Dict[str, Any] = {}
        if project_id:
            params["project_id"] = project_id

        resp = await self._client._http.post(
            "/api/v1/config/import",
            content=body,
            headers={"Content-Type": content_type},
            params=params,
        )
        raise_for_status(resp)
        return resp.json()

    async def import_from_file(
        self,
        path: Union[str, Path],
        project_id: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Async import from file."""
        path = Path(path)
        content = path.read_text(encoding="utf-8")
        return await self.import_config(content, project_id=project_id)
