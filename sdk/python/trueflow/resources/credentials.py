"""Resource for managing encrypted credentials."""

from typing import List, Dict, Any, Optional, Literal
from ..types import Credential, CredentialCreateResponse
from ..exceptions import raise_for_status


class CredentialsResource:
    """Management API resource for credentials (metadata only — no secrets exposed)."""

    def __init__(self, client):
        self._client = client

    def list(self, project_id: Optional[str] = None) -> List[Credential]:
        """List credential metadata for a project."""
        params = {}
        if project_id:
            params["project_id"] = project_id
        resp = self._client._http.get("/api/v1/credentials", params=params)
        raise_for_status(resp)
        return [Credential(**item) for item in resp.json()]

    def create(
        self,
        name: str,
        provider: str,
        *,
        # For builtin vault
        secret: Optional[str] = None,
        # For external vault
        vault_backend: Optional[Literal["builtin", "aws_kms", "aws_secrets_manager", "hashicorp_vault", "hashicorp_vault_kv", "azure_key_vault"]] = None,
        encrypted_secret_ref: Optional[str] = None,
        # For AWS Secrets Manager (secret_arn is stored in encrypted_secret_ref)
        # Common options
        project_id: Optional[str] = None,
        injection_mode: Optional[str] = None,
        injection_header: Optional[str] = None,
    ) -> CredentialCreateResponse:
        """
        Create a new credential.

        Three modes are supported:

        1. Builtin vault (default):
           Provide `secret` (plaintext API key) which will be encrypted by AILink.

           Example:
               credential = client.credentials.create(
                   name="prod-openai",
                   provider="openai",
                   secret="sk-proj-...",
               )

        2. AWS KMS:
           Set `vault_backend="aws_kms"` and provide `encrypted_secret_ref`
           (base64-encoded ciphertext from AWS KMS encrypt).

           Example:
               # First encrypt your key with your KMS:
               # aws kms encrypt --key-id $KMS_KEY --plaintext "sk-..." \\
               #     --output text --query CiphertextBlob

               credential = client.credentials.create(
                   name="prod-openai",
                   provider="openai",
                   vault_backend="aws_kms",
                   encrypted_secret_ref="AQICAHj...",
               )

        3. AWS Secrets Manager (Recommended):
           Set `vault_backend="aws_secrets_manager"` and provide `encrypted_secret_ref`
           as the secret ARN.

           Example:
               credential = client.credentials.create(
                   name="prod-openai",
                   provider="openai",
                   vault_backend="aws_secrets_manager",
                   encrypted_secret_ref="arn:aws:secretsmanager:us-east-1:123456789012:secret:prod/openai-key-xxx",
               )

        Returns a dict with the credential ``id`` and metadata.
        The secret is encrypted at rest and never returned.
        """
        payload: Dict[str, Any] = {
            "name": name,
            "provider": provider,
        }

        if vault_backend:
            payload["vault_backend"] = vault_backend

        if vault_backend in ("aws_kms", "hashicorp_vault", "hashicorp_vault_kv", "azure_key_vault", "aws_secrets_manager"):
            if not encrypted_secret_ref:
                raise ValueError(
                    f"encrypted_secret_ref is required for {vault_backend} backend"
                )
            payload["encrypted_secret_ref"] = encrypted_secret_ref
        else:
            # Builtin vault
            if not secret:
                raise ValueError("secret is required for builtin vault")
            payload["secret"] = secret

        if project_id:
            payload["project_id"] = project_id
        if injection_mode:
            payload["injection_mode"] = injection_mode
        if injection_header:
            payload["injection_header"] = injection_header

        resp = self._client._http.post("/api/v1/credentials", json=payload)
        raise_for_status(resp)
        return CredentialCreateResponse(**resp.json())


class AsyncCredentialsResource:
    """Async Management API resource for credentials."""

    def __init__(self, client):
        self._client = client

    async def list(self, project_id: Optional[str] = None) -> List[Credential]:
        """List credential metadata for a project."""
        params = {}
        if project_id:
            params["project_id"] = project_id
        resp = await self._client._http.get("/api/v1/credentials", params=params)
        raise_for_status(resp)
        return [Credential(**item) for item in resp.json()]

    async def create(
        self,
        name: str,
        provider: str,
        *,
        # For builtin vault
        secret: Optional[str] = None,
        # For external vault
        vault_backend: Optional[Literal["builtin", "aws_kms", "aws_secrets_manager", "hashicorp_vault", "hashicorp_vault_kv", "azure_key_vault"]] = None,
        encrypted_secret_ref: Optional[str] = None,
        # For AWS Secrets Manager (secret_arn is stored in encrypted_secret_ref)
        # Common options
        project_id: Optional[str] = None,
        injection_mode: Optional[str] = None,
        injection_header: Optional[str] = None,
    ) -> CredentialCreateResponse:
        """
        Create a new credential.

        Three modes are supported:

        1. Builtin vault (default):
           Provide `secret` (plaintext API key) which will be encrypted by AILink.

        2. AWS KMS:
           Set `vault_backend="aws_kms"` and provide `encrypted_secret_ref`
           (base64-encoded ciphertext from AWS KMS encrypt).

        3. AWS Secrets Manager (Recommended):
           Set `vault_backend="aws_secrets_manager"` and provide `encrypted_secret_ref`
           as the secret ARN.

        Returns a dict with the credential ``id`` and metadata.
        The secret is encrypted at rest and never returned.
        """
        payload: Dict[str, Any] = {
            "name": name,
            "provider": provider,
        }

        if vault_backend:
            payload["vault_backend"] = vault_backend

        if vault_backend in ("aws_kms", "hashicorp_vault", "hashicorp_vault_kv", "azure_key_vault", "aws_secrets_manager"):
            if not encrypted_secret_ref:
                raise ValueError(
                    f"encrypted_secret_ref is required for {vault_backend} backend"
                )
            payload["encrypted_secret_ref"] = encrypted_secret_ref
        else:
            # Builtin vault
            if not secret:
                raise ValueError("secret is required for builtin vault")
            payload["secret"] = secret

        if project_id:
            payload["project_id"] = project_id
        if injection_mode:
            payload["injection_mode"] = injection_mode
        if injection_header:
            payload["injection_header"] = injection_header

        resp = await self._client._http.post("/api/v1/credentials", json=payload)
        raise_for_status(resp)
        return CredentialCreateResponse(**resp.json())