# Secret References resource

"""Resource for managing external secret references."""

from typing import List, Optional, Dict, Any
from datetime import datetime

from pydantic import BaseModel, Field

from ..exceptions import raise_for_status


# ============================================================================
# Pydantic Models
# ============================================================================

class SecretReference(BaseModel):
    """A reference to an external secret in a vault backend.

    Secret references provide a workspace-scoped abstraction over external
    secrets stored in AWS Secrets Manager, HashiCorp Vault KV, or Azure Key Vault.
    """
    id: str
    project_id: str
    name: str
    description: Optional[str] = None
    vault_backend: str
    external_ref: str
    vault_config_id: Optional[str] = None
    provider: Optional[str] = None
    injection_mode: str
    injection_header: str
    allowed_team_ids: Optional[List[str]] = None
    allowed_user_ids: Optional[List[str]] = None
    last_accessed_at: Optional[datetime] = None
    last_rotated_at: Optional[datetime] = None
    version: Optional[str] = None
    is_active: bool
    created_at: datetime
    updated_at: datetime
    created_by: Optional[str] = None

    def __repr__(self) -> str:
        return f"SecretReference(id={self.id!r}, name={self.name!r}, vault_backend={self.vault_backend!r})"


class SecretReferenceCreateResponse(BaseModel):
    """Response from creating a secret reference."""
    id: Optional[str] = None
    name: Optional[str] = None
    vault_backend: Optional[str] = None
    external_ref: Optional[str] = None


class SecretFetchResponse(BaseModel):
    """Response from fetching a secret."""
    reference_id: str
    fetched_at: datetime
    message: str


# ============================================================================
# Sync Resource
# ============================================================================

class SecretReferencesResource:
    """Management API resource for secret references."""

    def __init__(self, client):
        self._client = client

    def list(
        self,
        *,
        vault_backend: Optional[str] = None,
        provider: Optional[str] = None,
        is_active: Optional[bool] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[SecretReference]:
        """List secret references.

        Args:
            vault_backend: Filter by vault backend type
                (aws_secrets_manager, hashicorp_vault, hashicorp_vault_kv, azure_key_vault).
            provider: Filter by provider (e.g., "openai", "anthropic").
            is_active: Filter by active status.
            limit: Maximum number of results (default 100, max 1000).
            offset: Pagination offset.

        Returns:
            List of SecretReference objects.
        """
        params: Dict[str, Any] = {}
        if vault_backend:
            params["vault_backend"] = vault_backend
        if provider:
            params["provider"] = provider
        if is_active is not None:
            params["is_active"] = is_active
        if limit is not None:
            params["limit"] = limit
        if offset is not None:
            params["offset"] = offset

        resp = self._client._http.get("/api/v1/secret-references", params=params)
        raise_for_status(resp)
        return [SecretReference(**item) for item in resp.json()]

    def create(
        self,
        name: str,
        vault_backend: str,
        external_ref: str,
        *,
        description: Optional[str] = None,
        vault_config_id: Optional[str] = None,
        provider: Optional[str] = None,
        injection_mode: Optional[str] = None,
        injection_header: Optional[str] = None,
        allowed_team_ids: Optional[List[str]] = None,
        allowed_user_ids: Optional[List[str]] = None,
    ) -> SecretReference:
        """Create a new secret reference.

        Args:
            name: Human-readable name for the secret reference.
            vault_backend: Vault backend type. One of: aws_secrets_manager,
                hashicorp_vault, hashicorp_vault_kv, azure_key_vault.
            external_ref: External reference to the secret.
                For AWS: ARN (e.g., "arn:aws:secretsmanager:us-east-1:123456789012:secret:my-secret")
                For HashiCorp Vault: path:key format (e.g., "secret/data/api:openai_key")
                For Azure Key Vault: secret URI.
            description: Optional description.
            vault_config_id: Optional vault config ID for authentication.
            provider: Provider this secret is for (e.g., "openai", "anthropic").
            injection_mode: How to inject the secret. One of: bearer (default),
                header, query, none.
            injection_header: Header name for injection (default: Authorization).
            allowed_team_ids: Team IDs allowed to access this secret.
            allowed_user_ids: User IDs allowed to access this secret.

        Returns:
            The created SecretReference.

        Example:
            ref = client.secret_references.create(
                name="prod-openai-key",
                vault_backend="aws_secrets_manager",
                external_ref="arn:aws:secretsmanager:us-east-1:123456789012:secret:prod/openai-key",
                provider="openai",
            )
        """
        payload: Dict[str, Any] = {
            "name": name,
            "vault_backend": vault_backend,
            "external_ref": external_ref,
        }
        if description is not None:
            payload["description"] = description
        if vault_config_id is not None:
            payload["vault_config_id"] = vault_config_id
        if provider is not None:
            payload["provider"] = provider
        if injection_mode is not None:
            payload["injection_mode"] = injection_mode
        if injection_header is not None:
            payload["injection_header"] = injection_header
        if allowed_team_ids is not None:
            payload["allowed_team_ids"] = allowed_team_ids
        if allowed_user_ids is not None:
            payload["allowed_user_ids"] = allowed_user_ids

        resp = self._client._http.post("/api/v1/secret-references", json=payload)
        raise_for_status(resp)
        return SecretReference(**resp.json())

    def get(self, id: str) -> SecretReference:
        """Get a secret reference by ID.

        Args:
            id: The secret reference ID.

        Returns:
            The SecretReference object.
        """
        resp = self._client._http.get(f"/api/v1/secret-references/{id}")
        raise_for_status(resp)
        return SecretReference(**resp.json())

    def update(
        self,
        id: str,
        *,
        name: Optional[str] = None,
        description: Optional[str] = None,
        external_ref: Optional[str] = None,
        vault_config_id: Optional[str] = None,
        provider: Optional[str] = None,
        injection_mode: Optional[str] = None,
        injection_header: Optional[str] = None,
        allowed_team_ids: Optional[List[str]] = None,
        allowed_user_ids: Optional[List[str]] = None,
        version: Optional[str] = None,
        is_active: Optional[bool] = None,
    ) -> SecretReference:
        """Update a secret reference.

        Args:
            id: The secret reference ID.
            name: New name for the secret reference.
            description: New description.
            external_ref: New external reference.
            vault_config_id: New vault config ID.
            provider: New provider.
            injection_mode: New injection mode.
            injection_header: New injection header.
            allowed_team_ids: New list of allowed team IDs.
            allowed_user_ids: New list of allowed user IDs.
            version: Secret version (for vaults that support versioning).
            is_active: Active status.

        Returns:
            The updated SecretReference.
        """
        payload: Dict[str, Any] = {}
        if name is not None:
            payload["name"] = name
        if description is not None:
            payload["description"] = description
        if external_ref is not None:
            payload["external_ref"] = external_ref
        if vault_config_id is not None:
            payload["vault_config_id"] = vault_config_id
        if provider is not None:
            payload["provider"] = provider
        if injection_mode is not None:
            payload["injection_mode"] = injection_mode
        if injection_header is not None:
            payload["injection_header"] = injection_header
        if allowed_team_ids is not None:
            payload["allowed_team_ids"] = allowed_team_ids
        if allowed_user_ids is not None:
            payload["allowed_user_ids"] = allowed_user_ids
        if version is not None:
            payload["version"] = version
        if is_active is not None:
            payload["is_active"] = is_active

        resp = self._client._http.put(f"/api/v1/secret-references/{id}", json=payload)
        raise_for_status(resp)
        return SecretReference(**resp.json())

    def delete(self, id: str) -> Dict[str, Any]:
        """Delete a secret reference.

        Args:
            id: The secret reference ID.

        Returns:
            Dict with 'id' and 'deleted' fields.
        """
        resp = self._client._http.delete(f"/api/v1/secret-references/{id}")
        raise_for_status(resp)
        return resp.json()

    def fetch(self, id: str) -> SecretFetchResponse:
        """Fetch and cache the secret from the external vault.

        This triggers an immediate fetch of the secret from the external vault.
        The secret is cached in Redis for subsequent lookups. The actual secret
        value is not returned in the response.

        Args:
            id: The secret reference ID.

        Returns:
            SecretFetchResponse with fetch status.
        """
        resp = self._client._http.post(f"/api/v1/secret-references/{id}/fetch")
        raise_for_status(resp)
        return SecretFetchResponse(**resp.json())


# ============================================================================
# Async Resource
# ============================================================================

class AsyncSecretReferencesResource:
    """Async Management API resource for secret references."""

    def __init__(self, client):
        self._client = client

    async def list(
        self,
        *,
        vault_backend: Optional[str] = None,
        provider: Optional[str] = None,
        is_active: Optional[bool] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[SecretReference]:
        """List secret references.

        Args:
            vault_backend: Filter by vault backend type
                (aws_secrets_manager, hashicorp_vault, hashicorp_vault_kv, azure_key_vault).
            provider: Filter by provider (e.g., "openai", "anthropic").
            is_active: Filter by active status.
            limit: Maximum number of results (default 100, max 1000).
            offset: Pagination offset.

        Returns:
            List of SecretReference objects.
        """
        params: Dict[str, Any] = {}
        if vault_backend:
            params["vault_backend"] = vault_backend
        if provider:
            params["provider"] = provider
        if is_active is not None:
            params["is_active"] = is_active
        if limit is not None:
            params["limit"] = limit
        if offset is not None:
            params["offset"] = offset

        resp = await self._client._http.get("/api/v1/secret-references", params=params)
        raise_for_status(resp)
        return [SecretReference(**item) for item in resp.json()]

    async def create(
        self,
        name: str,
        vault_backend: str,
        external_ref: str,
        *,
        description: Optional[str] = None,
        vault_config_id: Optional[str] = None,
        provider: Optional[str] = None,
        injection_mode: Optional[str] = None,
        injection_header: Optional[str] = None,
        allowed_team_ids: Optional[List[str]] = None,
        allowed_user_ids: Optional[List[str]] = None,
    ) -> SecretReference:
        """Create a new secret reference.

        Args:
            name: Human-readable name for the secret reference.
            vault_backend: Vault backend type. One of: aws_secrets_manager,
                hashicorp_vault, hashicorp_vault_kv, azure_key_vault.
            external_ref: External reference to the secret.
                For AWS: ARN (e.g., "arn:aws:secretsmanager:us-east-1:123456789012:secret:my-secret")
                For HashiCorp Vault: path:key format (e.g., "secret/data/api:openai_key")
                For Azure Key Vault: secret URI.
            description: Optional description.
            vault_config_id: Optional vault config ID for authentication.
            provider: Provider this secret is for (e.g., "openai", "anthropic").
            injection_mode: How to inject the secret. One of: bearer (default),
                header, query, none.
            injection_header: Header name for injection (default: Authorization).
            allowed_team_ids: Team IDs allowed to access this secret.
            allowed_user_ids: User IDs allowed to access this secret.

        Returns:
            The created SecretReference.
        """
        payload: Dict[str, Any] = {
            "name": name,
            "vault_backend": vault_backend,
            "external_ref": external_ref,
        }
        if description is not None:
            payload["description"] = description
        if vault_config_id is not None:
            payload["vault_config_id"] = vault_config_id
        if provider is not None:
            payload["provider"] = provider
        if injection_mode is not None:
            payload["injection_mode"] = injection_mode
        if injection_header is not None:
            payload["injection_header"] = injection_header
        if allowed_team_ids is not None:
            payload["allowed_team_ids"] = allowed_team_ids
        if allowed_user_ids is not None:
            payload["allowed_user_ids"] = allowed_user_ids

        resp = await self._client._http.post("/api/v1/secret-references", json=payload)
        raise_for_status(resp)
        return SecretReference(**resp.json())

    async def get(self, id: str) -> SecretReference:
        """Get a secret reference by ID.

        Args:
            id: The secret reference ID.

        Returns:
            The SecretReference object.
        """
        resp = await self._client._http.get(f"/api/v1/secret-references/{id}")
        raise_for_status(resp)
        return SecretReference(**resp.json())

    async def update(
        self,
        id: str,
        *,
        name: Optional[str] = None,
        description: Optional[str] = None,
        external_ref: Optional[str] = None,
        vault_config_id: Optional[str] = None,
        provider: Optional[str] = None,
        injection_mode: Optional[str] = None,
        injection_header: Optional[str] = None,
        allowed_team_ids: Optional[List[str]] = None,
        allowed_user_ids: Optional[List[str]] = None,
        version: Optional[str] = None,
        is_active: Optional[bool] = None,
    ) -> SecretReference:
        """Update a secret reference.

        Args:
            id: The secret reference ID.
            name: New name for the secret reference.
            description: New description.
            external_ref: New external reference.
            vault_config_id: New vault config ID.
            provider: New provider.
            injection_mode: New injection mode.
            injection_header: New injection header.
            allowed_team_ids: New list of allowed team IDs.
            allowed_user_ids: New list of allowed user IDs.
            version: Secret version (for vaults that support versioning).
            is_active: Active status.

        Returns:
            The updated SecretReference.
        """
        payload: Dict[str, Any] = {}
        if name is not None:
            payload["name"] = name
        if description is not None:
            payload["description"] = description
        if external_ref is not None:
            payload["external_ref"] = external_ref
        if vault_config_id is not None:
            payload["vault_config_id"] = vault_config_id
        if provider is not None:
            payload["provider"] = provider
        if injection_mode is not None:
            payload["injection_mode"] = injection_mode
        if injection_header is not None:
            payload["injection_header"] = injection_header
        if allowed_team_ids is not None:
            payload["allowed_team_ids"] = allowed_team_ids
        if allowed_user_ids is not None:
            payload["allowed_user_ids"] = allowed_user_ids
        if version is not None:
            payload["version"] = version
        if is_active is not None:
            payload["is_active"] = is_active

        resp = await self._client._http.put(f"/api/v1/secret-references/{id}", json=payload)
        raise_for_status(resp)
        return SecretReference(**resp.json())

    async def delete(self, id: str) -> Dict[str, Any]:
        """Delete a secret reference.

        Args:
            id: The secret reference ID.

        Returns:
            Dict with 'id' and 'deleted' fields.
        """
        resp = await self._client._http.delete(f"/api/v1/secret-references/{id}")
        raise_for_status(resp)
        return resp.json()

    async def fetch(self, id: str) -> SecretFetchResponse:
        """Fetch and cache the secret from the external vault.

        This triggers an immediate fetch of the secret from the external vault.
        The secret is cached in Redis for subsequent lookups. The actual secret
        value is not returned in the response.

        Args:
            id: The secret reference ID.

        Returns:
            SecretFetchResponse with fetch status.
        """
        resp = await self._client._http.post(f"/api/v1/secret-references/{id}/fetch")
        raise_for_status(resp)
        return SecretFetchResponse(**resp.json())